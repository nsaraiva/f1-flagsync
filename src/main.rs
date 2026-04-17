mod app;
mod ble;
mod menu;
mod protocol;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}

