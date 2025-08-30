// src/mount/moveslew.rs

use esp_idf_svc::hal::task::CriticalSection;
use log::{error, info};
use anyhow::Result;

use crate::mount::{app_state::PierSide, commands::Command, RustyScope};

/// /src/mount/moveslew.rs
/// 
/// this does all the starting and stopping of tracking
/// 

impl<'a> RustyScope<'a> {
    /// Starts sidereal tracking.
    pub fn move_focuser(&self, steps: i32) -> Result<()> {
        info!("Commanded to move focuser...");
        let cmd = Command::MoveFocuser{position: steps};
        self.send_command(cmd);
        Ok(())
        
    }
    pub fn execute_move_focuser(&self, target_position: i32) -> Result<()> {
        info!("Executing absolute focuser move to position: {}", target_position);
    
        //let mut planner = self.system.planner.lock().unwrap();
        let _cs = CriticalSection::new();
        let planner = unsafe { &*self.system.planner.0 };

        // 1. Get the current absolute position of the focuser.
        let current_position = planner.current_position_steps()[2];

        // 2. Calculate the RELATIVE number of steps to move.
        let delta_steps = target_position - current_position;
    
        // If we're already there, do nothing.
        if delta_steps == 0 {
            info!("Focuser is already at the target position.");
            return Ok(());
        }

        info!("Current focuser position: {}. Moving by {} steps.", current_position, delta_steps);
    
        // 3. Command the planner to move by the calculated RELATIVE delta.
       
        // --- Command the Planner (Single Critical Section) ---
    {
        let _cs = CriticalSection::new();
        let planner = unsafe { &mut *self.system.planner.0 };
        if let Err(e) = planner.move_steps([0, 0, delta_steps]) {
            error!("Failed to move focuser: {}", e);
            return Err(anyhow::anyhow!(e));
        }
    }
        Ok(())
    }

    //pub fn slew_to_coordinates(&self, target_ra: RightAscension, target_dec: Declination) -> Result<()> {
    //    info!("Commanded to slew to coordinates...");
    //    {
    //        let mut state = self.app_state.lock().unwrap();
    //        state.target_ra = target_ra;
    //        state.target_dec = target_dec;
    //    }
    //    let cmd = Command::SlewToTarget;
    //    self.send_command(cmd);
    //    Ok(())
    //}

    pub fn slew_to_target(&self) -> Result<()> {
        info!("Commanded to slew to target...");
        let cmd = Command::SlewToTarget;
        self.send_command(cmd);
        Ok(())
    }

    
    pub fn execute_slew_to_target(&self) -> Result<()> {
    info!("Executing 4-quadrant intelligent slew to target...");

    // --- 1. Get current state ---
    let (target_ra, target_dec, sync_offset, current_pos) = {
        let app_state = self.app_state.lock().unwrap();
        
        let _cs = CriticalSection::new();
        let planner = unsafe { &mut *self.system.planner.0 };
        (
            app_state.target_ra,
            app_state.target_dec,
            planner.sync_steps(),
            planner.current_position_steps(),
        )
    };
    let current_lst = self.system.time.get_lst();

    // --- 2. Calculate the four possible target positions ---
    
    // === Declination Targets ===
    // Target 1: The direct, "normal" path to the declination.
    let dec_steps_normal = self.dec_drivetrain.degrees_to_steps(target_dec.as_degrees());
    
    // Target 2: The "flipped" path, going over the pole and coming back.
    // We get the raw mount angle and flip it.
    let home_pole = self.dec_drivetrain.home_offset_degrees; // +90 or -90
    let flipped_dec_angle = if home_pole > 0.0 {
        // Northern Hemisphere: flip over North pole
        180.0 - target_dec.as_degrees()
    } else {
        // Southern Hemisphere: flip over South pole
        -180.0 - target_dec.as_degrees()
    };
    let dec_steps_flipped = self.dec_drivetrain.degrees_to_steps(flipped_dec_angle);

    // === Right Ascension Targets ===
    // These are the two pier-side positions for the target's Hour Angle.
    let target_ha_hours = target_ra.to_hour_angle_hours(current_lst);
    
    // Position for East side of pier
    let ra_mount_degrees_east = if target_ha_hours > 0.0 { (target_ha_hours - 12.0) * 15.0 } else { target_ha_hours * 15.0 };
    let ra_steps_east = self.ra_drivetrain.degrees_to_steps(ra_mount_degrees_east);
    
    // Position for West side of pier
    let ra_mount_degrees_west = if target_ha_hours < 0.0 { (target_ha_hours + 12.0) * 15.0 } else { target_ha_hours * 15.0 };
    let ra_steps_west = self.ra_drivetrain.degrees_to_steps(ra_mount_degrees_west);

    // --- 3. Define the four candidate destinations ---
    let candidates = [
        ("Normal (East/Normal)", [ra_steps_east, dec_steps_normal]),
        ("Pier Flip (West/Normal)", [ra_steps_west, dec_steps_normal]),
        ("Dec Flip (East/Flipped)", [ra_steps_east, dec_steps_flipped]),
        ("Both Flips (West/Flipped)", [ra_steps_west, dec_steps_flipped]),
    ];

    // --- 4. Calculate the "cost" (squared distance) to each candidate ---
    let costs = candidates.map(|(name, target)| {
        let cost = (target[0] as i64 - current_pos[0] as i64).pow(2) + 
                   (target[1] as i64 - current_pos[1] as i64).pow(2);
        (name, target, cost)
    });

    // --- 5. Find the candidate with the minimum cost ---
    // We add a safety check here to avoid pier crashes. A real mount would have limits.
    // For now, we will assume any path is valid. A better version would filter out
    // candidates that would cause a crash.
    let best_path = costs.iter().min_by_key(|(_, _, cost)| *cost)
                         .ok_or_else(|| anyhow::anyhow!("Could not determine a valid slew path."))?;

    let (chosen_name, chosen_target_steps, _) = best_path;
    let final_target_ra_steps = chosen_target_steps[0];
    let final_target_dec_steps = chosen_target_steps[1];

    // --- 6. Apply sync offset and command the planner ---
    let physical_target_ra = final_target_ra_steps + sync_offset[0];
    let physical_target_dec = final_target_dec_steps + sync_offset[1];
    let target_position = [physical_target_ra, physical_target_dec, 0];
    
    info!("--- 4-Quadrant Slew ---");
    info!("Current Steps: {:?}", current_pos);
    info!("Target Coords: RA {}, Dec {}", target_ra.as_hms_string(), target_dec.as_dms_string());
    info!("Candidates:");
    for (name, target, cost) in costs {
        info!("  - {}: Steps {:?}, Cost {}", name, target, cost);
    }
    info!("Best Path: {} -> {:?}", chosen_name, target_position);


    {
    let _cs = CriticalSection::new();
    let planner = unsafe { &mut *self.system.planner.0 };
    if let Err(e) = planner.move_to_steps(target_position) {
            return Err(anyhow::anyhow!(e));
        }
    }
    Ok(())
}
    

    /// Issues a command to start or stop moving a specific axis at a given rate.
    pub fn command_move_axis(&self, axis: u8, rate_dps: f32) -> Result<()> {
        info!("Move axis command issued: Axis {} at {} deg/sec", axis, rate_dps);
        let cmd = Command::MoveAxis { axis, rate_dps };
        self.send_command(cmd);
        Ok(())
    }

    /// Executes the MoveAxis command. This will be called by the commander task.
    /// It calculates the required steps/sec and commands the planner.
    pub fn execute_move_axis(&self, axis: u8, rate_dps: f32) -> Result<()> {
        // --- 1. Calculate the target rate vector ---
    let rate_sps = if rate_dps == 0.0 {
        0.0 // Handle stop case
    } else {
        match axis {
            0 => rate_dps as f64 * self.ra_drivetrain.steps_per_degree,
            1 => rate_dps as f64 * self.dec_drivetrain.steps_per_degree,
            2 => rate_dps as f64 * self.focus_drivetrain.steps_per_degree,
            _ => return Err(anyhow::anyhow!("Invalid axis number: {}", axis)),
        }
    };
    
    let mut new_rates_sps = [0.0, 0.0, 0.0];
    
    // If the rate is non-zero, we are starting a new move.
    // If the rate is zero, we must stop ALL axes, not just one.
    // This is because a client might send moveaxis(axis=1, rate=0) but the mount might
    // be moving on axis=0 due to tracking. The user expects "stop" to stop everything.
    if rate_dps != 0.0 {
        new_rates_sps[axis as usize] = rate_sps;
        // A manual move cancels astronomical tracking.
        self.app_state.lock().unwrap().is_tracking = false;
    } else {
        // A rate of 0.0 is a general stop for manual moves.
        info!("MoveAxis received rate 0.0, stopping all continuous motion.");
        // is_tracking will be set to false by execute_stop_tracking
        return self.execute_stop_tracking();
    }
    
    info!(
        "Setting continuous move rates to (steps/sec): {:?}",
        new_rates_sps
    );

    // --- 2. Command the planner ---
    // We only need one lock.
    // let _ = self.system.planner.lock().unwrap().track_rate_sps(new_rates_sps);
    // --- Command the Planner (Single Critical Section) ---
    {
        let _cs = CriticalSection::new();
        let planner = unsafe { &mut *self.system.planner.0 };
        if let Err(e) = planner.track_rate_sps(new_rates_sps) {
            // If the command fails, revert the state change.
            self.app_state.lock().unwrap().is_tracking = false;
            return Err(anyhow::anyhow!(e));
        }
    }

    Ok(())
    }

    


}

    