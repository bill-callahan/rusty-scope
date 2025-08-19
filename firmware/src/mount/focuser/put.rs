// src/mount/focuser/put.rs
use log::info;
use std::sync::Arc;

use crate::mount::{commands::Command, RustyScope};


// PUT handler for focuser
pub fn handle_focuser_put(
    scope: Arc<RustyScope<'static>>,
    endpoint: &str, 
    params: &std::collections::HashMap<String, String>
) -> Result<(), String> {
    let mut data = scope.app_state.lock().unwrap();
    match endpoint {
        "connected" => {
            let value = params.get("Connected")
            .ok_or("Missing Connected parameter")?
            .parse::<String>()
            .map_err(|_| "Invalid Connected value".to_string())?;
            // Implement your connect logic here
            
            if value == "True" {
                data.focconnecting = true;
                data.focconnected = true;
                data.focconnecting = false;
                info!("Focuser connected.");
            } else {
                data.focconnecting = true;
                data.focconnected = false;
                data.focconnecting = false;
                info!("Focuser disconnected.");
            }
            drop(data);
            Ok(())
        }
        "connect" => {
            // Implement connect logic
            data.focconnecting = true;
            data.focconnected = true;
            data.focconnecting = false;
            drop(data);
            Ok(())
        }
            
        "disconnect" => {
            // Implement disconnect logic
            data.focconnecting = true;
            data.focconnected = false;
            data.focconnecting = false;
            drop(data);
            Ok(())
        }
        "move" => {
            let position = params.get("Position")
            .ok_or("Missing Position parameter")?
            .parse::<i32>()
            .map_err(|_| "Invalid Position value".to_string())?;
    
            info!("Moving focuser to position: {}", position);
            let _ = scope.move_focuser(position);
            //scope.send_command(Command::MoveFocuser(position));  
            Ok(())
        }
        "halt" => {
            info!("Halt command received");

            // Implement halt logic
            // stop_motor(Motor::Z);
            //scope.system.planner.lock().unwrap().stop();
            scope.send_command(Command::StopMotion);
            Ok(())
        }
        "tempcomp" => {
            let enabled = params.get("TempComp")
                .ok_or("Missing TempComp parameter")?
                .parse::<bool>()
                .map_err(|_| "Invalid TempComp value".to_string())?;
            
            // Implement temp comp logic
            info!("TempComp set to: {}", enabled);
            //set_temp_comp(enabled);
            Ok(())
        }
        
        _ => Err(format!("Unsupported focuser PUT endpoint: {}", endpoint)),
    }
}



