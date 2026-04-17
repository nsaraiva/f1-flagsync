use anyhow::{anyhow, Result};
use btleplug::api::Manager as _;
use btleplug::platform::Manager;
use tokio::time::Duration;

use crate::ble;
use crate::menu::{self, MenuAction};
use crate::protocol;

const SEND_RETRY_ATTEMPTS: usize = 3;
const RETRY_DELAY: Duration = Duration::from_millis(700);

pub async fn run() -> Result<()> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;

    let central = adapters
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No Bluetooth adapters found"))?;

    let (device, device_name) = ble::choose_device(&central).await?;

    println!("\nConnecting to: {device_name}");
    ble::connect_until_ready(&device, RETRY_DELAY).await;

    ble::print_services_and_characteristics(&device);

    loop {
        let action = match menu::read_preset_action() {
            Ok(action) => action,
            Err(err) => {
                println!("Entrada invalida: {err}");
                continue;
            }
        };

        match action {
            MenuAction::Quit => {
                println!("Encerrando aplicacao...");
                break;
            }
            MenuAction::Send(value) => {
                let frame = protocol::build_bc_protocol_frame(value);
                println!(
                    "\nFrame BC gerado (valor={} / 0x{:04X}): {:02X?}",
                    value, value, frame
                );

                if let Err(err) = ble::send_with_retry(
                    &device,
                    &frame,
                    "Protocolo BC",
                    SEND_RETRY_ATTEMPTS,
                    RETRY_DELAY,
                )
                .await
                {
                    println!("Nao foi possivel enviar preset: {err}");
                }
            }
        }
    }

    Ok(())
}
