use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    bot_runner::run().await
}
