//! Explicit read-only upstream acceptance. No credentials are printed or persisted.
use local_agent_core::{
    domain::*,
    mcp::{Connection, Integration, Transport},
};
use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let preset = std::env::args().nth(1).unwrap_or_else(|| "context7".into());
    let url = match preset.as_str() {
        "context7" => "https://mcp.context7.com/mcp",
        "github" => "https://api.githubcopilot.com/mcp/",
        _ => return Err(AppError::new("ARGUMENT", "Choose context7 or github.")),
    };
    let config = Integration {
        id: id(),
        name: preset.clone(),
        preset: preset.clone(),
        enabled: true,
        approved: true,
        credential: None,
        transport: Transport::Http {
            url: url.into(),
            local: false,
            headers: BTreeMap::new(),
        },
    };
    let start = std::time::Instant::now();
    let connection = Connection::connect(&config, None, &CancellationToken::new()).await?;
    println!(
        "{}: protocol {}, {} indexed tools, {:.2}s",
        preset,
        connection.protocol,
        connection.tools.len(),
        start.elapsed().as_secs_f64()
    );
    if preset == "context7" {
        let tool = connection
            .tools
            .iter()
            .find(|tool| tool.name == "resolve-library-id")
            .ok_or_else(|| AppError::new("MCP_TOOL", "Upstream resolver changed."))?;
        let result = connection
            .call(
                tool,
                serde_json::json!({"libraryName":"tauri","query":"Current Tauri tray icon API"}),
            )
            .await;
        connection.close().await?;
        let result = result?;
        println!(
            "resolver result: {}",
            local_agent_core::security::bounded(&result.to_string(), 700)
        );
    } else {
        connection.close().await?;
    }
    Ok(())
}
