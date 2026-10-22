//! Authenticated read-only QA using only the explicitly configured isolated app integration.
use local_agent_core::{domain::*, mcp, store::Store};
use serde_json::{json, Value};
use std::path::Path;
use tokio_util::sync::CancellationToken;

async fn call(connection: &mcp::Connection, name: &str, arguments: Value) -> Result<Value> {
    let tool = connection
        .tools
        .iter()
        .find(|tool| {
            tool.name == name && tool.effect == local_agent_core::capabilities::Effect::Read
        })
        .ok_or_else(|| {
            AppError::new(
                "MCP_TOOL",
                format!("Verified read tool unavailable: {name}"),
            )
        })?;
    let result = connection.call(tool, arguments).await?;
    if result["isError"] == true {
        return Err(AppError::new(
            "MCP_TOOL_ERROR",
            format!("GitHub returned an error for {name}; no call was retried."),
        ));
    }
    println!("PASS: {name} read returned bounded evidence");
    let envelope: Value = serde_json::from_str(result["evidence"].as_str().unwrap_or("{}"))?;
    if let Some(content) = envelope.get("structuredContent") {
        return Ok(content.clone());
    }
    envelope["content"]
        .as_array()
        .and_then(|blocks| {
            blocks.iter().find_map(|block| {
                block["text"]
                    .as_str()
                    .and_then(|text| serde_json::from_str(text).ok())
            })
        })
        .ok_or_else(|| {
            AppError::new(
                "MCP_RESULT",
                "Expected structured GitHub result; inspect the current upstream format.",
            )
        })
}
fn number(value: &Value) -> Option<u64> {
    match value {
        Value::Object(fields) => fields
            .get("number")
            .and_then(Value::as_u64)
            .or_else(|| fields.values().find_map(number)),
        Value::Array(items) => items.iter().find_map(number),
        _ => None,
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let store = Store::open(&workspace.join(".local-data/mcp-completion-ui"))?;
    let integration = mcp::list(&store)?.into_iter().find(|config|config.preset == "github" && config.approved && config.enabled && config.credential.is_some())
        .ok_or_else(||AppError::new("MCP_AUTH","Save the GitHub credential in the isolated QA app first. No other credential source is used."))?;
    let connection = mcp::Connection::connect(
        &integration,
        mcp::secret(&store, &integration)?,
        &CancellationToken::new(),
    )
    .await?;
    let result = async {
        call(&connection,"search_repositories",json!({"query":"repo:Guardianec/local-agent","perPage":1})).await?;
        let issues = call(&connection,"search_issues",json!({"query":"repo:github/github-mcp-server is:issue","perPage":1})).await?;
        let issue = number(&issues).ok_or_else(||AppError::new("MCP_RESULT","No issue identifier returned."))?;
        call(&connection,"issue_read",json!({"method":"get_comments","owner":"github","repo":"github-mcp-server","issue_number":issue})).await?;
        let prs = call(&connection,"search_pull_requests",json!({"query":"repo:github/github-mcp-server is:pr","perPage":1,"fields":["number","title","state"]})).await?;
        let pr = number(&prs).ok_or_else(||AppError::new("MCP_RESULT","No PR identifier returned."))?;
        call(&connection,"pull_request_read",json!({"method":"get_reviews","owner":"github","repo":"github-mcp-server","pullNumber":pr,"perPage":1})).await?;
        call(&connection,"actions_list",json!({"method":"list_workflow_runs","owner":"Guardianec","repo":"local-agent","perPage":2})).await?;
        Ok(())
    }.await;
    connection.close().await?;
    result
}
