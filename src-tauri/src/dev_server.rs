//! Approved project code remains host execution; the app owns its bounded Job.
use crate::{
    commands::{self, CommandPlan, CommandResult},
    domain::*,
};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct DevServer {
    pub url: String,
    cancel: CancellationToken,
    worker: Option<JoinHandle<Result<CommandResult>>>,
}
impl Drop for DevServer {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
impl DevServer {
    pub async fn start(
        mut plan: CommandPlan,
        script: &str,
        parent: &CancellationToken,
    ) -> Result<Self> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        plan.environment.insert("PORT".into(), port.to_string());
        plan.environment.insert("HOST".into(), "127.0.0.1".into());
        plan.environment.insert("BROWSER".into(), "none".into());
        plan.timeout = Duration::from_secs(720);
        if script.trim_start().starts_with("vite") {
            plan.args.extend([
                "--".into(),
                "--host".into(),
                "127.0.0.1".into(),
                "--port".into(),
                port.to_string(),
                "--strictPort".into(),
            ]);
        } else if script.trim_start().starts_with("next dev") {
            plan.args.extend([
                "--".into(),
                "--hostname".into(),
                "127.0.0.1".into(),
                "--port".into(),
                port.to_string(),
            ]);
        }
        let cancel = parent.child_token();
        let child = cancel.clone();
        let worker = tokio::task::spawn_blocking(move || commands::execute(plan, child));
        let server = Self {
            url: format!("http://127.0.0.1:{port}/"),
            cancel,
            worker: Some(worker),
        };
        let mut failure=AppError::new("DEV_SERVER_PORT","The script did not listen on its assigned PORT within ten seconds. Use an externally prepared disposable server and supply its URL.");
        for _ in 0..100 {
            if parent.is_cancelled() {
                failure = AppError::new("CANCELLED", "Development server start stopped.");
                break;
            }
            if server.worker.as_ref().is_some_and(|w| w.is_finished()) {
                failure = AppError::new(
                    "DEV_SERVER_EXIT",
                    "The development script exited before its loopback port was ready.",
                );
                break;
            }
            if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .is_ok()
            {
                return Ok(server);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        server.close().await?;
        Err(failure)
    }
    pub async fn close(mut self) -> Result<()> {
        self.cancel.cancel();
        if let Some(worker) = self.worker.take() {
            worker.await.map_err(|e| {
                AppError::new("DEV_SERVER_WORKER", "Development server cleanup failed.").detail(e)
            })??;
        }
        Ok(())
    }
}
