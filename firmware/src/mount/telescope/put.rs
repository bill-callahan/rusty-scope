// src/mount/telescope/put.rs
use std::sync::Arc;



use hifitime::Epoch;
use log::info;

use crate::mount::{commands::{Command, GuideDirection}, tracking::TrackingRateType, Altitude, Azimuth, Declination, RightAscension, RustyScope};



// Example PUT handler for telescope
pub fn handle_telescope_put(
    scope: Arc<RustyScope<'static>>,
    endpoint: &str, 
    params: &std::collections::HashMap<String, String>
) -> Result<(), String> {
    let mut data = scope.app_state.lock().unwrap();
    match endpoint {
        "connect" => {
            // Implement your connect logic here
            data.telconnecting = true;
            data.telconnected = true;
            data.telconnecting = false;
            
            Ok(())
        }
        "disconnect" => {
            // Implement your disconnect logic here
            data.telconnecting = true;
            data.telconnected = false;
            data.telconnecting = false;
            
            Ok(())
        }
        "connected" => {
            // Implement your connected logic here
            data.telconnecting = false;
            data.telconnected = true;
            
            Ok(())
        }
        "rightascensionrate" => {
            let rightascensionrate = params.get("RightAscensionRate")
                .ok_or("Missing RightAscensionRate parameter")?
                .parse::<f32>()
                .map_err(|_| "Invalid RightAscensionRate value".to_string())?;

            // Implement your right ascension rate update logic here
            //let factor = 0.9972695677;  // the number of UTC seconds in a sidereal second
            let factor: f32 = 0.9972696;  // the number of UTC seconds in a sidereal second
            let ra_rate = rightascensionrate * factor;
            data.ra_rate = ra_rate;   
            //set_telescope_rightascensionrate(rightascensionrate);
            Ok(())
        }
        "declinationrate" => {
            let declinationrate = params.get("DeclinationRate")
                .ok_or("Missing DeclinationRate parameter")?
                .parse::<f32>()
                .map_err(|_| "Invalid DeclinationRate value".to_string())?;
            // Implement your declination rate update logic here
            data.dec_rate = declinationrate;
            //set_telescope_declinationrate(declinationrate);
            Ok(())
        }
        "guideraterightascension" => {
            let guideraterightascension = params.get("GuideRateRightAscension")
                .ok_or("Missing GuideRateRightAscension parameter")?
                .parse::<f32>()
                .map_err(|_| "Invalid GuideRateRightAscension value".to_string())?;
            // Implement your guide rate right ascension update logic here
            data.guide_ra_rate = guideraterightascension;
            Ok(())
        }
        "guideratedeclination" => {
            let guideratedeclination = params.get("GuideRateDeclination")
                .ok_or("Missing GuideRateDeclination parameter")?
                .parse::<f32>()
                .map_err(|_| "Invalid GuideRateDeclination value".to_string())?;
            // Implement your guide rate declination update logic here
            data.guide_dec_rate = guideratedeclination;
            Ok(())
        }

        "trackingrate" => {
            // Check if we are in the grace period.
            if let Some(last_special_track) = data.last_non_sidereal_track_time {
                let grace_period = hifitime::Duration::from_seconds(5.0);
                let now = Epoch::now().unwrap();
                if now - last_special_track < grace_period {
                    // We are within 5 seconds of starting a special track.
                    // IGNORE this incoming `trackingrate` command.
                    info!("[QUIRKS MODE] Ignoring trackingrate change from client during grace period.");
                    return Ok(());
                }
            }
            let trackingrate = params.get("TrackingRate")
                .ok_or("Missing Tracking parameter")?
                .parse::<i32>()
                .map_err(|_| "Invalid TrackingRate value".to_string())?;
            
            // Implement your tracking update logic here
            data.tracking_rate = trackingrate;
            Ok(())
        }
        "tracking" => {
            let tracking = params.get("Tracking")
                .ok_or("Missing Tracking parameter")?
                .to_lowercase()
                .parse::<bool>()
                .map_err(|_| "Invalid Tracking value".to_string())?;

            // Implement your tracking update logic here
            if data.is_parked {
                 return Err("Mount is parked. Cannot set tracking.".to_string());
            }
            // if tracking == true {
            //     //scope.start_tracking().map_err(|e| format!("Failed to start tracking: {}", e))?;
            //     scope.send_command(Command::StartTracking);
            // } else {
            //     //scope.stop_tracking().map_err(|e| format!("Failed to stop tracking: {}", e))?;
            //     scope.send_command(Command::StopTracking);
            // }
            
            // When the "tracking" endpoint is called, we read the *currently stored*
            // tracking rate from the app_state and bundle it into the command.
            let current_rate = TrackingRateType::from(data.tracking_rate);

            // Drop the lock before sending the command
            drop(data); 

            // Issue the new, stateful command
            scope.send_command(Command::SetTracking { enabled: tracking, rate: current_rate });
            
            Ok(())
        }
        "slewtoaltazasync" => { // The Alpaca spec calls for the async version
            // --- 1. Parse and Validate Input Parameters ---
            let altitude_deg = params.get("Altitude")
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or("Invalid or missing 'Altitude' (degrees) parameter".to_string())?;

            let azimuth_deg = params.get("Azimuth")
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or("Invalid or missing 'Azimuth' (degrees) parameter".to_string())?;

            // --- 2. Create Type-Safe Coordinate Objects ---
            let target_alt = Altitude::new(altitude_deg);
            let target_az = Azimuth::new(azimuth_deg);

            // --- 3. Perform the Coordinate Conversion ---
            // We need the current time and site info to do the conversion.
            let current_lst = scope.system.time.get_lst();
            // We can get the latitude from the existing `data` lock.
            let site_latitude = data.site_info.latitude.as_degrees();

            let (target_ra, target_dec) = RustyScope::horizontal_to_equatorial(
                &target_alt,
                &target_az,
                current_lst,
                site_latitude,
            );
            
            // --- 4. Set the new target and issue the standard slew command ---
            // Now we modify the shared state with the new target.
            data.target_ra = target_ra;
            data.target_dec = target_dec;
            
            // Release the lock before calling the command function.
            drop(data);
            
            // Issue the generic SlewToTarget command. The commander will pick it up.
            scope.slew_to_target().map_err(|e| e.to_string())?;
                 
            Ok(())
        }
        "targetrightascension" => {
                let ra = params.get("TargetRightAscension")
                .ok_or("Missing RA parameter")?
                .parse::<f64>()
                .map_err(|_| "Invalid RA value".to_string())?;

                data.target_ra = RightAscension::new_from_hours(ra);
                drop(data);
                Ok(())
        }
        "targetdeclination" => {
            let dec = params.get("TargetDeclination")
                .ok_or("Missing Dec parameter")?
                .parse::<f64>()
                .map_err(|_| "Invalid Dec value".to_string())?;

            data.target_dec = Declination::new(dec);
            drop(data);
            Ok(())
        }
        "slewtocoordinatesasync" => {
            let ra = params.get("RightAscension")
                .ok_or("Missing RA parameter")?
                .parse::<f64>()
                .map_err(|_| "Invalid RA value".to_string())?;

            let dec = params.get("Declination")
                .ok_or("Missing Dec parameter")?
                .parse::<f64>()
                .map_err(|_| "Invalid Dec value".to_string())?;

            // Implement slew logic
            if data.is_parked {
                return Err("Mount is parked.  Cannot slew.".to_string());
            }
            data.target_ra = RightAscension::new_from_hours(ra);
            data.target_dec = Declination::new(dec);
            drop(data);
            //let _ = scope.slew_to_target();
            scope.send_command(Command::SlewToTarget);
            Ok(())
        }
        "slewtotargetasync" => {
            // Implement slew to target logic
            if data.is_parked {
                return Err("Mount is parked.  Cannot slew.".to_string());
            }
            drop(data);
            // let _ = scope.slew_to_target();
            scope.send_command(Command::SlewToTarget);
            Ok(())
        }
        "synctocoordinates" => {
            // --- 1. Parse and Validate Input Parameters ---
            // The Alpaca spec states that RA is passed in hours and Dec in degrees.
            let ra_hours = params.get("RightAscension")
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or("Invalid or missing 'RightAscension' (hours) parameter".to_string())?;

            let dec_degrees = params.get("Declination")
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or("Invalid or missing 'Declination' (degrees) parameter".to_string())?;

            // --- 2. Create Type-Safe Coordinate Objects ---
            // This is the only place we need to create these objects.
            // The `new` methods will automatically validate and normalize the inputs.
            let sync_ra = RightAscension::new_from_hours(ra_hours);
            let sync_dec = Declination::new(dec_degrees);

            // --- 3. Issue the Command ---
            // We can drop the lock on `data` as we don't need to modify it.
            drop(data);

            // Call the command method with the correct, type-safe arguments.
            scope.command_sync_to_coordinates(sync_ra, sync_dec)
                 .map_err(|e| e.to_string())?;
                 
            Ok(())
        }
        "synctotarget" => {
            // --- 1. Get the Current Target Coordinates ---
            // We need to read the target RA and Dec from the shared app_state.
            // We clone them so we can release the lock immediately.
            let target_ra = data.target_ra;
            let target_dec = data.target_dec;
            
            // --- 2. Issue the Command ---
            // Release the lock on `data` before calling the command function.
            drop(data);

            // Call the same command function as synctocoordinates, but pass in
            // the target coordinates we just retrieved.
            scope.command_sync_to_coordinates(target_ra, target_dec)
                 .map_err(|e| e.to_string())?;
                 
            Ok(())
        }
        "moveaxis" => {
            // --- ADD THIS GUARD ---
            if data.is_parked {
                 return Err("Mount is parked. Cannot move axis.".to_string());
            }

            // 1. Parse and Validate Input Parameters
            // The Alpaca spec uses 0 for RA, 1 for Dec.
            let axis = params.get("Axis")
                .and_then(|s| s.parse::<u8>().ok())
                .ok_or("Invalid or missing 'Axis' parameter".to_string())?;

            // Rate is in degrees per second. Can be positive or negative.
            let rate_dps = params.get("Rate")
                .and_then(|s| s.parse::<f32>().ok())
                .ok_or("Invalid or missing 'Rate' parameter".to_string())?;

            info!("Received MoveAxis request: Axis {}, Rate: {} deg/s", axis, rate_dps);
            
            // 2. Drop the lock and issue the command
            drop(data);
            scope.command_move_axis(axis, rate_dps)
                 .map_err(|e| e.to_string())?;

            Ok(())
        }
        "abortslew" => {
            // Implement abort slew logic
            //scope.execute_stop_motion();
            scope.send_command(Command::StopMotion);
            Ok(())
        }
        "park" => {
            // Implement park logic
            //park();
            let _ = scope.command_park();
            Ok(())
        }
        "unpark" => {
            // Implement unpark logic
            drop(data);
            scope.command_unpark()?;
            Ok(())
        }
        "findhome" => {
            // Implement find home logic
            //find_home();
            let _ = scope.command_home();
            Ok(())
        }
        "pulseguide" => {
            // Implement pulse guide logic
            //if scope.app_state.lock().unwrap().is_parked {
            //     return Err("Mount is parked. Cannot pulse guide.".to_string());
            //}

            let duration = params.get("Duration")
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or("Invalid or missing 'Duration' parameter".to_string())?;

            let direction_str = params.get("Direction")
                .ok_or("Missing 'Direction' parameter".to_string())?;

            // The Alpaca spec uses integers for direction.
            let direction = match direction_str.parse::<i32>() {
                Ok(0) => GuideDirection::North,
                Ok(1) => GuideDirection::South,
                Ok(2) => GuideDirection::East,
                Ok(3) => GuideDirection::West,
                _ => return Err(format!("Invalid Direction value: {}", direction_str)),
            };
            info!("[API]Received PulseGuide request: Duration {}s, Direction {}", duration, direction_str);
            scope.command_pulse_guide(direction, duration)
                 .map_err(|e| e.to_string())?;
            Ok(())
        }
        "calibrate_tracking" => {
            // Implement calibrate tracking logic
            //calibrate_tracking();
            scope.send_command(Command::SlewToTarget);
            scope.send_command(Command::CalibrateTracking);
            Ok(())
        }
        _ => Err(format!("Unsupported telescope PUT endpoint: {}", endpoint)),
    }
}