// src/mount/management/put.rs

use std::sync::Arc;
use esp_idf_svc::hal::delay::FreeRtos;
use log::info;

use crate::mount::RustyScope;
use super::config::{TelescopeConfig, FocuserConfig};

// This is the main handler function for all management PUT requests.
pub fn handle_management_put(scope: Arc<RustyScope<'static>>, endpoint: &str, body: Option<&str>) -> Result<(), String> {
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
        "v1/configure/telescope" => {
            let body = body.ok_or_else(|| "Missing body for telescope configuration".to_string())?;
            let config: TelescopeConfig = serde_json::from_str(body)
                .map_err(|e| format!("Failed to parse telescope config: {}", e))?;
            
            info!("Received telescope configuration: {:?}", config);
            
            // Now, update the application state with the new config
            //let mut app_state = scope.app_state.lock().unwrap();
            //app_state.config.motorx_spr = config.motorx_spr;
            //app_state.config.motorx_microsteps = config.motorx_microsteps;
            //app_state.config.motorx_gratio = config.motorx_gratio;
            //app_state.config.motory_spr = config.motory_spr;
            //app_state.config.motory_microsteps = config.motory_microsteps;
            //app_state.config.motory_gratio = config.motory_gratio;
            //app_state.config.mount_type = config.mount_type;
            //app_state.config.ra_reversed = config.ra_reversed;
            //app_state.config.dec_reversed = config.dec_reversed;
            
            // Save the updated config to persistent storage
            //scope.save_persistent_config(&app_state.config)?;

            Ok(())
        }
        "v1/configure/focuser" => {
            let body = body.ok_or_else(|| "Missing body for focuser configuration".to_string())?;
            let config: FocuserConfig = serde_json::from_str(body)
                .map_err(|e| format!("Failed to parse focuser config: {}", e))?;

            info!("Received focuser configuration: {:?}", config);

            //let mut app_state = scope.app_state.lock().unwrap();
            //app_state.config.motorz_spr = config.motorz_spr;
            //app_state.config.motorz_microsteps = config.motorz_microsteps;
            //app_state.config.motorz_gratio = config.motorz_gratio;
            //app_state.config.focuser_max_step = config.focuser_max_step;
            //app_state.config.focuser_step_size = config.focuser_step_size;
            //app_state.config.focuser_reversed = config.focuser_reversed;

            //scope.save_persistent_config(&app_state.config)?;
            
            Ok(())
        }
        _ => Err(format!("Unsupported management PUT endpoint: {}", endpoint)),
    }
}