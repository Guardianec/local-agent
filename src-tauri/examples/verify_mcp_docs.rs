//! Anonymous, bounded documentation acceptance for this project's libraries.
use local_agent_core::{domain::*, mcp};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() -> Result<()> {
    let config = mcp::Integration {
        id: id(),
        name: "Context7".into(),
        preset: "context7".into(),
        enabled: true,
        approved: true,
        credential: None,
        transport: mcp::Transport::Http {
            url: "https://mcp.context7.com/mcp".into(),
            local: false,
            headers: Default::default(),
        },
    };
    let connection = mcp::Connection::connect(&config, None, &CancellationToken::new()).await?;
    let result = async {
        let resolve = connection
            .tools
            .iter()
            .find(|tool| tool.name == "resolve-library-id")
            .ok_or_else(|| AppError::new("MCP_TOOL", "Resolver unavailable."))?;
        let query = connection
            .tools
            .iter()
            .find(|tool| tool.name == "query-docs")
            .ok_or_else(|| AppError::new("MCP_TOOL", "Documentation query unavailable."))?;
        let id_pattern = regex::Regex::new(r"Context7-compatible library ID:\s*(/[^\s]+)").unwrap();
        for (library, question) in [
            ("tauri", "Tauri 2 system tray icon API"),
            ("react", "React useEffect cleanup function"),
            ("ollama", "Ollama chat tool_calls function arguments"),
            (
                "modelcontextprotocol rust-sdk",
                "Rust rmcp Streamable HTTP client",
            ),
        ] {
            let resolved = connection
                .call(resolve, json!({"libraryName":library,"query":question}))
                .await?;
            if resolved["isError"] == true {
                return Err(AppError::new(
                    "MCP_TOOL_ERROR",
                    format!("Resolver failed for {library}."),
                ));
            }
            let envelope: Value =
                serde_json::from_str(resolved["evidence"].as_str().unwrap_or("{}"))?;
            let text = envelope["content"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            let id = id_pattern
                .captures(&text)
                .and_then(|captures| captures.get(1))
                .ok_or_else(|| {
                    AppError::new("MCP_RESULT", format!("No library ID for {library}."))
                })?
                .as_str();
            let answer = connection
                .call(query, json!({"libraryId":id,"query":question}))
                .await?;
            if answer["isError"] == true {
                return Err(AppError::new(
                    "MCP_TOOL_ERROR",
                    format!("Documentation query failed for {library}."),
                ));
            }
            assert!(!answer["evidence"].as_str().unwrap_or("").is_empty());
            println!(
                "PASS: {library} resolver + query-docs ({id}); bounded={} bytes; truncated={}",
                answer["evidence"].as_str().unwrap().len(),
                answer["truncated"]
            );
        }
        Ok(())
    }
    .await;
    connection.close().await?;
    result
}
