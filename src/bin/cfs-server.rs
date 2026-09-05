use anyhow::Result;
use cfs_expression_merger::web_server::{WebServerConfig, run_local_web_server};

#[tokio::main]
async fn main() -> Result<()> {
    run_local_web_server(WebServerConfig::from_env()?).await
}
