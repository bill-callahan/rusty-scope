//! # WiFi Connection Service
//!
//! This module handles the initialization and connection of the ESP32's WiFi peripheral.
//! It provides a simple, blocking function to connect to a pre-configured WiFi network
//! using credentials supplied at compile-time.

use esp_idf_svc::{
    eventloop::EspSystemEventLoop, 
    hal::modem::Modem, 
    nvs::{EspNvsPartition, NvsDefault}, 
    sys::EspError, 
    wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi}
};
use log::info;


// The SSID (network name) and password for the WiFi network.
// These are embedded in the firmware at compile time from environment variables.
const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASS");


/// Initializes and connects to the WiFi network.
///
/// This function performs the following steps in a blocking manner:
/// 1. Creates a new `EspWifi` instance.
/// 2. Configures it as a WiFi client with the SSID and password from the environment.
/// 3. Starts the WiFi peripheral.
/// 4. Connects to the access point.
/// 5. Waits until the network interface is up and has an IP address.
///
/// The SSID and password must be provided at compile time via the `WIFI_SSID` and
/// `WIFI_PASS` environment variables.
///
/// # Arguments
///
/// * `modem` - The ESP32 modem peripheral, required for creating the WiFi driver.
/// * `sys_loop` - The system event loop service, used for handling WiFi events.
/// * `nvs` - A handle to the default NVS partition, used by the WiFi driver for persistence.
///
/// # Returns
///
/// A `Result` containing a `Box<EspWifi<'static>>` on success. This box must be kept
/// in scope for the lifetime of the application to keep the WiFi connection alive.
/// On failure, it returns an `EspError`.
pub fn wifi_create(
    modem: Modem<'_>,
    sys_loop: EspSystemEventLoop,
    nvs: EspNvsPartition<NvsDefault>,
) -> Result<Box<EspWifi<'_>>, EspError> {
    // Create the underlying ESP-IDF WiFi driver.
    let mut esp_wifi = EspWifi::new(modem, sys_loop.clone(), Some(nvs.clone()))?;

    // Wrap the driver in a blocking helper for simpler synchronous operation.
    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sys_loop.clone())?;

    // Configure the WiFi client with credentials from the environment.
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        password: PASSWORD.try_into().unwrap(),
        ..Default::default()
    }))?;

    match wifi.start() {
        Ok(_) => info!("Wifi started"),
        Err(err) => info!("Wifi start failed: {:?}", err),
    }
    match wifi.connect() {
        Ok(_) => info!("Wifi connected"),
        Err(err) => info!("Wifi connection failed: {:?}", err),
    }

    match wifi.wait_netif_up() {
        Ok(_) => info!("Wifi netif up.  Ready to go... "),
        Err(err) => info!("Wifi netif up failed: {:?}", err),
    }

    // Return the `EspWifi` instance. The caller must keep this alive.
    Ok(Box::new(esp_wifi))
}