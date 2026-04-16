use anyhow::{anyhow, Result};
use btleplug::api::{
    Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use std::io::{self, Write};
use tokio::time::{sleep, Duration};

const SCAN_DURATION: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<()> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;

    let central = adapters
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No Bluetooth adapters found"))?;

    let devices_with_props = scan_devices(&central).await?;
    let selected_index = read_device_selection(devices_with_props.len())?;

    let (device, device_name) = devices_with_props
        .into_iter()
        .nth(selected_index)
        .ok_or_else(|| anyhow!("Index out of range"))?;

    println!("\nConnecting to: {device_name}");
    device.connect().await?;
    device.discover_services().await?;

    print_services_and_characteristics(&device);

    Ok(())
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

            // Use displayed index based only on selectable devices.
            let display_index = devices_with_props.len();
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

    Ok(devices_with_props)
}

fn read_device_selection(device_count: usize) -> Result<usize> {
    print!("\nSelect a device by index: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let selected_index: usize = input.trim().parse().map_err(|_| anyhow!("Invalid index"))?;

    if selected_index >= device_count {
        return Err(anyhow!("Index out of range"));
    }

    Ok(selected_index)
}

fn print_services_and_characteristics(device: &Peripheral) {
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

