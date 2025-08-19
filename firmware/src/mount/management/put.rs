// src/mount/management/put.rs

use std::sync::Arc;
use esp_idf_svc::hal::delay::FreeRtos;
use log::info;

use crate::mount::RustyScope;

// This is the main handler function for all management PUT requests.
// For now, it only handles reboot and factoryreset.
pub fn handle_management_put(scope: Arc<RustyScope<'static>>, endpoint: &str) -> Result<(), String> {
    match endpoint {
        "v1/reboot" => {
            info!("[API] Reboot command received from web UI. Rebooting in 3 seconds...");
            
            // Spawn a new, detached thread to handle the reboot.
            // This allows the server to send the "OK" HTTP response back to the
            // browser *before* the device actually restarts.
            std::thread::spawn(|| {
                // Give the network stack a moment.
                FreeRtos::delay_ms(3000);
                // This function safely restarts the ESP32.
                unsafe { esp_idf_svc::sys::esp_restart() };
            });

            // Return Ok to signal the command was accepted.
            Ok(())
        }
        "v1/factoryreset" => {
            info!("[API] Factory Reset command received. Erasing config and rebooting...");
            
            // Perform the delete operation. We can ignore a failure if the file doesn't exist.
            let _ = scope.delete_persistent_config();
            // You might also want a separate function to delete WiFi credentials from NVS.

            // Spawn a thread to reboot, just like above.
            std::thread::spawn(|| {
                FreeRtos::delay_ms(3000);
                unsafe { esp_idf_svc::sys::esp_restart() };
                
            });

            Ok(())
        }
        _ => Err(format!("Unsupported management PUT endpoint: {}", endpoint)),
    }
}