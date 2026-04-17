use anyhow::{anyhow, Result};
use btleplug::api::{
    Central, CharPropFlags, Characteristic, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Peripheral};
use std::io::{self, Write};
use tokio::time::{sleep, Duration};

const SCAN_DURATION: Duration = Duration::from_secs(5);

enum DeviceSelection {
    Rescan,
    Device(usize),
}

pub async fn choose_device(central: &Adapter) -> Result<(Peripheral, String)> {
    loop {
        let devices_with_props = scan_devices(central).await?;

        match read_device_selection(devices_with_props.len())? {
            DeviceSelection::Rescan => continue,
            DeviceSelection::Device(selected_index) => {
                let selected = devices_with_props
                    .into_iter()
                    .nth(selected_index)
                    .ok_or_else(|| anyhow!("Index out of range"))?;
                return Ok(selected);
            }
        }
    }
}

pub async fn connect_until_ready(device: &Peripheral, retry_delay: Duration) {
    loop {
        match ensure_connected(device).await {
            Ok(()) => return,
            Err(err) => {
                println!("Falha ao conectar/discover: {err}. Tentando novamente...");
                sleep(retry_delay).await;
            }
        }
    }
}

pub fn print_services_and_characteristics(device: &Peripheral) {
    println!("\nServices and characteristics:\n");

    for service in device.services() {
        println!("Service UUID: {}", service.uuid);

        for characteristic in service.characteristics {
            let props = characteristic.properties;

            println!("  Characteristic UUID: {}", characteristic.uuid);
            println!("    Properties: {:?}", props);

            if props.contains(CharPropFlags::WRITE) {
                println!("    -> Supports WRITE");
            }

            if props.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE) {
                println!("    -> Supports WRITE WITHOUT RESPONSE");
            }

            if props.contains(CharPropFlags::NOTIFY) {
                println!("    -> Supports NOTIFY");
            }

            if props.contains(CharPropFlags::READ) {
                println!("    -> Supports READ");
            }

            println!();
        }
    }
}

pub async fn send_with_retry(
    device: &Peripheral,
    payload: &[u8],
    label: &str,
    retry_attempts: usize,
    retry_delay: Duration,
) -> Result<()> {
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 1..=retry_attempts {
        if attempt > 1 {
            println!(
                "\nTentativa {}/{}: reconectando e reenviando comando...",
                attempt, retry_attempts
            );
            reconnect_device(device, retry_delay).await?;
            sleep(retry_delay).await;
        }

        if let Err(err) = ensure_connected(device).await {
            println!("Falha ao garantir conexao: {err}");
            last_error = Some(err);
            continue;
        }

        match write_payload(device, payload, label).await {
            Ok(()) => return Ok(()),
            Err(err) => {
                println!("Falha no envio: {err}");
                last_error = Some(err);
            }
        }
    }

    Err(anyhow!(
        "Falha apos {} tentativas: {}",
        retry_attempts,
        last_error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "erro desconhecido".to_string())
    ))
}

async fn scan_devices(central: &Adapter) -> Result<Vec<(Peripheral, String)>> {
    println!("Starting scan...");
    central.start_scan(ScanFilter::default()).await?;
    sleep(SCAN_DURATION).await;

    // Stop scanning before listing results to avoid extra BLE traffic.
    let _ = central.stop_scan().await;

    let peripherals = central.peripherals().await?;

    if peripherals.is_empty() {
        println!("No Bluetooth devices found.");
        return Err(anyhow!("No Bluetooth devices found."));
    }

    let mut devices_with_props: Vec<(Peripheral, String)> = Vec::with_capacity(peripherals.len());

    for p in peripherals {
        let props = p.properties().await?;

        if let Some(props) = props {
            let name = props.local_name.unwrap_or_else(|| "Unknown".to_string());
            let addr = props.address;
            let rssi = props
                .rssi
                .map(|v| v.to_string())
                .unwrap_or_else(|| "N/A".to_string());

            let display_index = devices_with_props.len() + 1;
            println!(
                "[{}] Device: {}, Address: {}, RSSI: {}",
                display_index, name, addr, rssi
            );

            devices_with_props.push((p, name));
        }
    }

    if devices_with_props.is_empty() {
        return Err(anyhow!("No devices with readable properties were found"));
    }

    println!("[0] Scan novamente");

    Ok(devices_with_props)
}

fn read_device_selection(device_count: usize) -> Result<DeviceSelection> {
    print!("\nSelecione um dispositivo pelo indice (0 para scan novamente): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let selected_index: usize = input.trim().parse().map_err(|_| anyhow!("Invalid index"))?;

    if selected_index == 0 {
        return Ok(DeviceSelection::Rescan);
    }

    if selected_index > device_count {
        return Err(anyhow!("Index out of range"));
    }

    Ok(DeviceSelection::Device(selected_index - 1))
}

async fn ensure_connected(device: &Peripheral) -> Result<()> {
    let connected = device
        .is_connected()
        .await
        .map_err(|e| anyhow!("Nao foi possivel checar conexao: {e}"))?;

    if connected {
        return Ok(());
    }

    device.connect().await?;
    device.discover_services().await?;
    Ok(())
}

async fn reconnect_device(device: &Peripheral, retry_delay: Duration) -> Result<()> {
    if device.is_connected().await.unwrap_or(false) {
        let _ = device.disconnect().await;
        sleep(retry_delay).await;
    }

    device.connect().await?;
    device.discover_services().await?;
    Ok(())
}

async fn write_payload(device: &Peripheral, payload: &[u8], label: &str) -> Result<()> {
    let characteristic = find_writable_characteristic(device)?;
    let write_type = write_type_for_characteristic(&characteristic)?;

    println!(
        "\nEnviando {} ({} bytes) para a characteristic {}...",
        label,
        payload.len(),
        characteristic.uuid
    );

    device.write(&characteristic, payload, write_type).await?;

    println!("Payload enviado com sucesso.");
    Ok(())
}

fn find_writable_characteristic(device: &Peripheral) -> Result<Characteristic> {
    device
        .services()
        .iter()
        .flat_map(|service| service.characteristics.iter())
        .find(|characteristic| {
            characteristic.properties.contains(CharPropFlags::WRITE)
                || characteristic
                    .properties
                    .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
        })
        .cloned()
        .ok_or_else(|| anyhow!("Nenhuma characteristic com WRITE encontrada"))
}

fn write_type_for_characteristic(characteristic: &Characteristic) -> Result<WriteType> {
    if characteristic
        .properties
        .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
    {
        return Ok(WriteType::WithoutResponse);
    }

    if characteristic.properties.contains(CharPropFlags::WRITE) {
        return Ok(WriteType::WithResponse);
    }

    Err(anyhow!("Characteristic nao suporta escrita"))
}
