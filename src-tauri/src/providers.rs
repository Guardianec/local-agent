use crate::{domain::*, security, store::Store};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{future::Future, pin::Pin, time::Duration};
use tokio_util::sync::CancellationToken;

pub struct ChatResponse {
    pub message: Value,
    pub tokens: u32,
    pub prompt_tokens: u32,
    pub tokens_per_second: Option<f64>,
    pub eval_ms: f64,
    pub time_to_first_token_ms: Option<f64>,
    pub total_ms: f64,
}
#[derive(Clone, Copy)]
pub enum ThinkingPolicy {
    Minimal,
    Default,
}
#[derive(Clone, Copy)]
pub struct ChatOptions {
    pub context: u32,
    pub thinking: ThinkingPolicy,
}
impl From<u32> for ChatOptions {
    fn from(context: u32) -> Self {
        Self {
            context,
            thinking: ThinkingPolicy::Minimal,
        }
    }
}
pub trait ModelProvider: Send + Sync {
    fn chat<'a>(
        &'a self,
        model: &'a str,
        messages: &'a [Value],
        tools: &'a [Value],
        options: ChatOptions,
        cancel: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<ChatResponse>> + Send + 'a>>;
}
#[derive(Clone)]
pub struct Ollama {
    client: reqwest::Client,
    base_url: String,
}
impl Ollama {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(3))
                .build()
                .map_err(|e| {
                    AppError::new(
                        "RUNTIME_CLIENT",
                        "Could not prepare the local model connection.",
                    )
                    .detail(e)
                })?,
            base_url: "http://127.0.0.1:11434".into(),
        })
    }
    #[cfg(test)]
    pub fn test_endpoint(endpoint: String) -> Self {
        let mut provider = Self::new().unwrap();
        provider.base_url = endpoint;
        provider
    }
    async fn get(&self, path: &str) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}{path}", self.base_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(runtime_error)?;
        let response = response.error_for_status().map_err(runtime_error)?;
        let bytes = response.bytes().await.map_err(runtime_error)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(AppError::new(
                "RUNTIME_LIMIT",
                "The local runtime returned too much metadata.",
            ));
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
    async fn show(&self, name: &str) -> Result<Value> {
        let response = self
            .client
            .post(format!("{}/api/show", self.base_url))
            .json(&json!({"model":name}))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(runtime_error)?
            .error_for_status()
            .map_err(runtime_error)?;
        let bytes = response.bytes().await.map_err(runtime_error)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(AppError::new(
                "RUNTIME_LIMIT",
                "Model metadata exceeded its limit.",
            ));
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub async fn health(&self, store: &Store) -> RuntimeInfo {
        let installed = installed_path().is_some();
        let version = match self.get("/api/version").await {
            Ok(v) => v["version"].as_str().map(String::from),
            Err(_) => {
                return RuntimeInfo {
                    status: "offline".into(),
                    version: None,
                    installed,
                    models: vec![],
                    message: "Local model service isn't running. Open Ollama, then try again."
                        .into(),
                    gpu: None,
                    calibrations: vec![],
                    memory: crate::lifecycle::memory_info(),
                }
            }
        };
        let tags = match self.get("/api/tags").await {
            Ok(v) => v,
            Err(e) => {
                return RuntimeInfo {
                    status: "error".into(),
                    version,
                    installed,
                    models: vec![],
                    message: e.message,
                    gpu: None,
                    calibrations: vec![],
                    memory: crate::lifecycle::memory_info(),
                }
            }
        };
        let running = self.get("/api/ps").await.unwrap_or(json!({"models":[]}));
        let mut models = Vec::new();
        if let Some(items) = tags["models"].as_array() {
            for item in items.iter().take(100) {
                let name = item["name"].as_str().unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }
                let digest = item["digest"].as_str().unwrap_or("").to_string();
                let loaded = running["models"]
                    .as_array()
                    .and_then(|a| a.iter().find(|r| r["name"] == name));
                let probe = store
                    .get::<Value>(
                        "probes",
                        &format!("{}:{}", digest, version.as_deref().unwrap_or("")),
                    )
                    .ok();
                let local = item["size"].as_u64().unwrap_or(0) > 0
                    && !name.contains(":cloud")
                    && item.get("remote_host").is_none()
                    && item.get("remote_model").is_none();
                let metadata_key = format!("{}:{}", digest, version.as_deref().unwrap_or(""));
                let metadata = if let Ok(v) = store.get::<Value>("model_metadata", &metadata_key) {
                    v
                } else {
                    let capabilities = self
                        .show(&name)
                        .await
                        .ok()
                        .and_then(|v| v["capabilities"].as_array().cloned())
                        .unwrap_or_default();
                    let v = json!({"vision":capabilities.contains(&json!("vision")),"thinking":capabilities.contains(&json!("thinking"))});
                    if !capabilities.is_empty() {
                        let _ = store.put("model_metadata", &metadata_key, &v);
                    }
                    v
                };
                models.push(ModelInfo {
                    name,
                    digest,
                    size: item["size"].as_f64().unwrap_or(0.0),
                    parameter_size: item["details"]["parameter_size"]
                        .as_str()
                        .unwrap_or("Unknown")
                        .into(),
                    quantization: item["details"]["quantization_level"]
                        .as_str()
                        .unwrap_or("Unknown")
                        .into(),
                    loaded: loaded.is_some(),
                    vram: loaded.and_then(|v| v["size_vram"].as_f64()),
                    loaded_bytes: loaded.and_then(|v| v["size"].as_f64()),
                    capability: probe
                        .and_then(|v| v["status"].as_str().map(String::from))
                        .unwrap_or("unknown".into()),
                    local,
                    vision: metadata["vision"].as_bool().unwrap_or(false),
                    thinking: metadata["thinking"].as_bool().unwrap_or(false),
                });
            }
        }
        RuntimeInfo {
            status: "ready".into(),
            version,
            installed,
            models,
            message: "Connected to Ollama on this PC".into(),
            gpu: if self.base_url == "http://127.0.0.1:11434" {
                gpu_info().await
            } else {
                None
            },
            calibrations: store.list_limit("calibrations", 100).unwrap_or_default(),
            memory: crate::lifecycle::memory_info(),
        }
    }
    pub async fn prepare(&self, name: &str, context: u32, cancel: CancellationToken) -> Result<()> {
        let request = self.client.post(format!("{}/api/generate", self.base_url))
            .json(&json!({"model":name,"prompt":"","stream":false,"keep_alive":"5m","options":{"num_ctx":context}}))
            .timeout(Duration::from_secs(240));
        let response = tokio::select! { _=cancel.cancelled()=>return Err(AppError::new("CANCELLED","Model loading stopped.")), result=request.send()=>result.map_err(runtime_error)? };
        if !response.status().is_success() {
            return Err(AppError::new(
                "MODEL_LOAD_FAILED",
                "The model could not load. Free GPU memory or choose a smaller model.",
            ));
        }
        self.control_complete(response, cancel).await
    }
    pub async fn unload(&self, name: &str) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&json!({"model":name,"keep_alive":0,"stream":false}))
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(runtime_error)?
            .error_for_status()
            .map_err(runtime_error)?;
        self.control_complete(response, CancellationToken::new())
            .await
    }
    async fn control_complete(
        &self,
        response: reqwest::Response,
        cancel: CancellationToken,
    ) -> Result<()> {
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();
        loop {
            let next = tokio::select! {_=cancel.cancelled()=>return Err(AppError::new("CANCELLED","Model transition stopped.")),r=tokio::time::timeout(Duration::from_secs(240),stream.next())=>r.map_err(|_|AppError::new("MODEL_LOAD_FAILED","The model transition did not finish in time."))?};
            let Some(chunk) = next else {
                break;
            };
            let chunk = chunk.map_err(runtime_error)?;
            if body.len() + chunk.len() > 65536 {
                return Err(AppError::new(
                    "RUNTIME_LIMIT",
                    "Model transition status exceeded its limit.",
                ));
            }
            body.extend(chunk);
        }
        let result: Value = serde_json::from_slice(&body)?;
        if result["done"] != true || result.get("error").is_some() {
            return Err(AppError::new(
                "MODEL_LOAD_FAILED",
                "Ollama did not confirm that the model transition completed.",
            ));
        }
        Ok(())
    }
    pub async fn pull(&self, name: &str, store: &Store, cancel: CancellationToken) -> Result<()> {
        if crate::registry::identity(name).is_none() {
            return Err(AppError::new(
                "MODEL_ID",
                "Only a model from the local catalog can be installed here.",
            ));
        }
        let mut progress = ModelInstallProgress {
            model: name.into(),
            status: "Connecting to download".into(),
            completed: 0.0,
            total: None,
            done: false,
            error: None,
        };
        store.put("services", "model-install", &progress)?;
        let response = tokio::select! {_=cancel.cancelled()=>return Err(AppError::new("CANCELLED","Model download stopped.")),r=self.client.post(format!("{}/api/pull",self.base_url)).json(&json!({"model":name,"stream":true})).send()=>r.map_err(runtime_error)?};
        if !response.status().is_success() {
            return Err(AppError::new("MODEL_DOWNLOAD","Ollama could not start this download. Check disk space, the model tag and your connection."));
        }
        let mut stream = response.bytes_stream();
        let mut pending = Vec::new();
        let mut last = std::time::Instant::now() - Duration::from_secs(1);
        let mut success = false;
        loop {
            let chunk = tokio::select! {_=cancel.cancelled()=>return Err(AppError::new("CANCELLED","Model download stopped. Ollama can reuse completed layers when you retry.")),r=tokio::time::timeout(Duration::from_secs(120),stream.next())=>r.map_err(|_|AppError::new("MODEL_DOWNLOAD","The download stopped responding. Retry when your connection is available."))?};
            let Some(chunk) = chunk else {
                break;
            };
            pending.extend(chunk.map_err(runtime_error)?);
            if pending.len() > 1024 * 1024 {
                return Err(AppError::new(
                    "RUNTIME_LIMIT",
                    "Download status exceeded its limit.",
                ));
            }
            while let Some(end) = pending.iter().position(|c| *c == b'\n') {
                let line = pending.drain(..=end).collect::<Vec<_>>();
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                let v: Value = serde_json::from_slice(&line)?;
                if let Some(error) = v["error"].as_str() {
                    return Err(AppError::new("MODEL_DOWNLOAD","The download did not finish. Check disk space and your connection, then retry.").detail(security::bounded(&security::redact(error),1000)));
                }
                progress.status =
                    security::bounded(v["status"].as_str().unwrap_or("Downloading"), 160);
                progress.completed = v["completed"].as_f64().unwrap_or(0.0);
                progress.total = v["total"].as_f64();
                success |= v["status"] == "success";
                progress.done = success;
                if last.elapsed() > Duration::from_millis(500) || success {
                    store.put("services", "model-install", &progress)?;
                    last = std::time::Instant::now();
                }
            }
        }
        if !success {
            return Err(AppError::new(
                "MODEL_DOWNLOAD",
                "The download ended before installation was confirmed. Retry to resume.",
            ));
        }
        let show = self.show(name).await?;
        let spec = crate::registry::identity(name).unwrap();
        if show["details"]["quantization_level"] != spec.quantization
            || show.get("remote_host").is_some()
            || show.get("remote_model").is_some()
        {
            return Err(AppError::new("MODEL_IDENTITY","The installed model did not match the requested local quantization. No fallback was selected."));
        }
        Ok(())
    }
    pub async fn delete_model(&self, name: &str) -> Result<()> {
        self.client
            .delete(format!("{}/api/delete", self.base_url))
            .json(&json!({"model":name}))
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(runtime_error)?
            .error_for_status()
            .map_err(runtime_error)?;
        Ok(())
    }
    pub async fn probe(
        &self,
        name: &str,
        store: &Store,
        cancel: CancellationToken,
    ) -> Result<ModelInfo> {
        let health = self.health(store).await;
        let mut model=health.models.into_iter().find(|m|m.name==name && m.local).ok_or_else(||AppError::new("MODEL_MISSING","Choose a locally installed model. Cloud and remote models are unavailable here."))?;
        let show = self.show(name).await?;
        if show.get("remote_host").is_some()
            || show.get("remote_model").is_some()
            || show["details"]["parameter_size"].as_str().is_none()
        {
            return Err(AppError::new(
                "MODEL_LOCALITY",
                "The model could not be verified as local. No project content was sent.",
            ));
        }
        let tools = vec![
            json!({"type":"function","function":{"name":"read_probe","description":"Read synthetic probe data. Call this exactly once.","parameters":{"type":"object","properties":{},"additionalProperties":false,"required":[]}}}),
        ];
        let mut messages = vec![
            json!({"role":"system","content":"This is a tool compatibility check. Call read_probe with an empty object, then repeat the returned proof string in your final answer. Do not guess the proof."}),
            json!({"role":"user","content":"Read the synthetic probe using the tool."}),
        ];
        let mut first = self
            .chat(name, &messages, &tools, 4096.into(), cancel.clone())
            .await?;
        let mut repairs = 0;
        loop {
            let calls = first.message["tool_calls"].as_array();
            let valid = calls.is_some_and(|calls| {
                calls.len() == 1
                    && calls[0]["function"]["name"] == "read_probe"
                    && calls[0]["function"]["arguments"] == json!({})
            });
            if valid {
                break;
            }
            if repairs >= 1 {
                return Err(AppError::new(
                    "TOOLS_UNSUPPORTED",
                    "This model returned incompatible tool calls after a bounded correction.",
                )
                .detail(security::bounded(
                    &security::redact(&serde_json::to_string(&first.message["tool_calls"])?),
                    1500,
                )));
            }
            repairs += 1;
            messages.push(first.message);
            messages.push(json!({"role":"tool","tool_name":"read_probe","content":"Tool arguments were rejected; nothing executed. read_probe accepts exactly an empty object {}. Do not add a parameters key, wrapper or string. Call read_probe once with {}."}));
            first = self
                .chat(name, &messages, &tools, 4096.into(), cancel.clone())
                .await?;
        }
        messages.push(first.message);
        let proof = id().replace('-', "")[..12].to_string();
        messages.push(json!({"role":"tool","tool_name":"read_probe","content":serde_json::to_string(&json!({"proof":proof}))?}));
        let second = self
            .chat(name, &messages, &tools, 4096.into(), cancel)
            .await?;
        if second.message["tool_calls"]
            .as_array()
            .is_some_and(|calls| !calls.is_empty())
            || !second.message["content"]
                .as_str()
                .unwrap_or("")
                .contains(&proof)
        {
            return Err(AppError::new(
                "TOOLS_UNSUPPORTED",
                "This model could not complete a tool-result follow-up.",
            )
            .detail(security::bounded(
                &security::redact(
                    second.message["content"]
                        .as_str()
                        .unwrap_or("No visible answer"),
                ),
                500,
            )));
        }
        model.capability = "verified".into();
        store.put(
            "probes",
            &format!("{}:{}", model.digest, health.version.unwrap_or_default()),
            &json!({"status":"verified","version":1,"at":now(),"repairs":repairs}),
        )?;
        Ok(model)
    }
}
impl ModelProvider for Ollama {
    fn chat<'a>(
        &'a self,
        model: &'a str,
        messages: &'a [Value],
        tools: &'a [Value],
        options: ChatOptions,
        cancel: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<ChatResponse>> + Send + 'a>> {
        Box::pin(async move {
            let started = std::time::Instant::now();
            let mut request = json!({"model":model,"messages":messages,"tools":tools,"stream":true,"keep_alive":"5m","options":{"num_ctx":options.context,"num_predict":if matches!(options.thinking,ThinkingPolicy::Default){4096}else{2048},"temperature":0.1}});
            if let Ok(show) = self.show(model).await {
                if show["thinking"]["values"]
                    .as_array()
                    .is_some_and(|values| values.contains(&json!(false)))
                    && matches!(options.thinking, ThinkingPolicy::Minimal)
                {
                    request["think"] = json!(false);
                }
            }
            let response = tokio::select! {_=cancel.cancelled()=>return Err(AppError::new("CANCELLED","The task was stopped.")),r=self.client.post(format!("{}/api/chat",self.base_url)).json(&request).timeout(Duration::from_secs(600)).send()=>r.map_err(runtime_error)?};
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::new("INFERENCE_FAILED","The local model could not answer. Check model availability and available memory.").detail(format!("{status}: {}",security::bounded(&security::redact(&body),1000))));
            }
            let mut stream = response.bytes_stream();
            let mut decoder = Decoder::default();
            let mut first = true;
            let mut first_token = None;
            loop {
                let next = tokio::select! {_=cancel.cancelled()=>return Err(AppError::new("CANCELLED","The task was stopped.")),r=tokio::time::timeout(Duration::from_secs(if first{240}else{90}),stream.next())=>r.map_err(|_|AppError::new("INFERENCE_TIMEOUT","The local model stopped responding. Your proposed changes are retained."))?};
                match next {
                    None => break,
                    Some(chunk) => {
                        first = false;
                        decoder.push(&chunk.map_err(runtime_error)?)?;
                        if first_token.is_none()
                            && (!decoder.content.is_empty()
                                || !decoder.thinking.is_empty()
                                || !decoder.calls.is_empty())
                        {
                            first_token = Some(started.elapsed().as_secs_f64() * 1000.0);
                        }
                    }
                }
            }
            let mut result = decoder.finish()?;
            result.time_to_first_token_ms = first_token;
            result.total_ms = started.elapsed().as_secs_f64() * 1000.0;
            Ok(result)
        })
    }
}
fn runtime_error(e: reqwest::Error) -> AppError {
    AppError::new(
        if e.is_timeout() {
            "RUNTIME_TIMEOUT"
        } else {
            "RUNTIME_OFFLINE"
        },
        if e.is_timeout() {
            "The local model service did not respond in time."
        } else {
            "Local model service isn't running or the connection was interrupted."
        },
    )
    .detail(e)
}
pub fn installed_path() -> Option<std::path::PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    [
        std::path::PathBuf::from(&local).join("Programs/Ollama/ollama app.exe"),
        std::path::PathBuf::from(local).join("Programs/Ollama/ollama.exe"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

#[derive(Default)]
pub struct Decoder {
    pending: Vec<u8>,
    total: usize,
    content: String,
    thinking: String,
    calls: Vec<Value>,
    done: bool,
    tokens: u32,
    prompt: u32,
    speed: Option<f64>,
    eval_ms: f64,
}
impl Decoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<()> {
        self.total += bytes.len();
        if self.total > 2 * 1024 * 1024 {
            return Err(AppError::new(
                "INFERENCE_LIMIT",
                "The model response exceeded its output limit.",
            ));
        }
        self.pending.extend(bytes);
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            if end > 512 * 1024 {
                return Err(AppError::new(
                    "INFERENCE_LIMIT",
                    "A model message exceeded its limit.",
                ));
            }
            let line = self.pending.drain(..=end).collect::<Vec<_>>();
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            self.line(&line)?;
        }
        if self.pending.len() > 512 * 1024 {
            return Err(AppError::new(
                "INFERENCE_LIMIT",
                "A model message exceeded its limit.",
            ));
        }
        Ok(())
    }
    fn line(&mut self, line: &[u8]) -> Result<()> {
        if self.done {
            return Err(AppError::new(
                "INFERENCE_PROTOCOL",
                "The runtime sent data after the final message.",
            ));
        }
        let v: Value = serde_json::from_slice(line)?;
        if let Some(error) = v["error"].as_str() {
            return Err(AppError::new(
                "INFERENCE_FAILED",
                "The local runtime reported a model error.",
            )
            .detail(security::redact(error)));
        }
        self.content
            .push_str(v["message"]["content"].as_str().unwrap_or(""));
        self.thinking
            .push_str(v["message"]["thinking"].as_str().unwrap_or(""));
        if let Some(calls) = v["message"]["tool_calls"].as_array() {
            self.calls.extend(calls.clone());
            if self.calls.len() > 4 {
                return Err(AppError::new(
                    "TOOL_LIMIT",
                    "The model requested too many actions at once.",
                ));
            }
        }
        if v["done"].as_bool() == Some(true) {
            if v["done_reason"] == "length" {
                return Err(AppError::new("INFERENCE_LIMIT","The model reached its response limit before finishing. No tool calls from this response were executed."));
            }
            self.done = true;
            self.tokens = v["eval_count"]
                .as_u64()
                .unwrap_or((self.content.len() + self.thinking.len()) as u64 / 2)
                as u32;
            self.prompt = v["prompt_eval_count"].as_u64().unwrap_or(0) as u32;
            let duration = v["eval_duration"].as_f64().unwrap_or(0.0);
            self.eval_ms = duration / 1e6;
            if duration > 0.0 {
                self.speed = Some(self.tokens as f64 / (duration / 1e9));
            }
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<ChatResponse> {
        if !self.pending.iter().all(u8::is_ascii_whitespace) {
            let line = std::mem::take(&mut self.pending);
            self.line(&line)?;
        }
        if !self.done {
            return Err(AppError::new(
                "INFERENCE_INTERRUPTED",
                "The model response ended early. No incomplete tool requests were executed.",
            ));
        }
        let mut message = json!({"role":"assistant","content":self.content});
        if !self.thinking.is_empty() {
            message["thinking"] = json!(self.thinking);
        }
        if !self.calls.is_empty() {
            message["tool_calls"] = json!(self.calls);
        }
        Ok(ChatResponse {
            message,
            tokens: self.tokens,
            prompt_tokens: self.prompt,
            tokens_per_second: self.speed,
            eval_ms: self.eval_ms,
            time_to_first_token_ms: None,
            total_ms: 0.0,
        })
    }
}

pub async fn gpu_info() -> Option<GpuInfo> {
    use std::os::windows::process::CommandExt;
    let executable =
        std::path::PathBuf::from(std::env::var_os("SystemRoot")?).join("System32/nvidia-smi.exe");
    if !executable.is_file() {
        return None;
    }
    let mut command = std::process::Command::new(executable);
    command
        .args([
            "--query-gpu=name,memory.total,memory.free",
            "--format=csv,noheader,nounits",
        ])
        .creation_flags(0x08000000);
    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(2), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() || output.stdout.len() > 8192 {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let fields = text
        .lines()
        .next()?
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    if fields.len() != 3 {
        return None;
    }
    Some(GpuInfo {
        name: fields[0].into(),
        total_bytes: fields[1].parse::<f64>().ok()? * 1048576.0,
        free_bytes: fields[2].parse::<f64>().ok()? * 1048576.0,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_every_byte_and_require_terminal() {
        let bytes="{\"message\":{\"content\":\"héllo\"},\"done\":false}\n{\"message\":{},\"done\":true,\"eval_count\":3}\n".as_bytes();
        let mut d = Decoder::default();
        for b in bytes {
            d.push(&[*b]).unwrap();
        }
        let result = d.finish().unwrap();
        assert_eq!(result.message["content"], "héllo");
        assert_eq!(result.tokens, 3);
        let mut d = Decoder::default();
        d.push(b"{\"message\":{\"content\":\"partial\"}}\n")
            .unwrap();
        assert!(d.finish().is_err());
    }
}
