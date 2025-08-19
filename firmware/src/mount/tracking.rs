/// /src/mount/tracking.rs
/// 
/// this does all the starting and stopping of tracking
/// 

use esp_idf_svc::hal::{delay::FreeRtos, task::CriticalSection};
use hifitime::Epoch;
use log::info;
use anyhow::Result;

use crate::{gps::pps_sync::get_clock_correction_factor, mount::{commands::{Command, GuideDirection}, Declination, RightAscension, RustyScope}};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackingRateType {
    Sidereal = 0,
    Lunar = 1,
    Solar = 2,
    King = 3,
}

impl From<i32> for TrackingRateType {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Lunar,
            2 => Self::Solar,
            3 => Self::King,
            _ => Self::Sidereal,
        }
    }
}



impl<'a> RustyScope<'a> {
    /// Starts sidereal tracking.
    pub fn start_tracking(&self) -> Result<()> {
        info!("Commanded to start sidereal tracking...");
        let cmd = Command::StartTracking;
        self.send_command(cmd);
        Ok(())
    }
    pub fn execute_start_tracking(&self) -> Result<()> {
    info!("Executing Start Tracking command...");
    let mut app_state = self.app_state.lock().unwrap();

    // Set the tracking flag immediately.
    app_state.is_tracking = true;

    // --- The Correction Pipeline ---

    // 1. Get the BASE mechanical rate (in steps/sec for sidereal tracking).
    let base_sidereal_rate_sps = self.ra_drivetrain.get_sidereal_tracking_rate_sps();

    // 2. Determine the desired ASTRONOMICAL rate from the app_state.
    let tracking_mode: TrackingRateType = app_state.tracking_rate.into();
    
    // Calculate the astronomical adjustment factor.
    let astronomical_factor = match tracking_mode {
        TrackingRateType::Solar => 23.9344699 / 24.0, // Sidereal day vs Solar day
        TrackingRateType::Lunar => 0.966, // Approximation for lunar rate
        // The King rate is slightly faster than sidereal to compensate for atmospheric refraction.
        // A common factor is to be about 15 arcseconds per DAY faster than sidereal.
        // Sidereal day = 86164.09 seconds.
        // King day = 86164.09 - 15 = 86149.09 seconds.
        // Factor = Sidereal Day / King Day
        TrackingRateType::King => 86164.09 / (86164.09 - 15.0), // Approx 1.000174
        _ => 1.0, // Default to Sidereal (no adjustment)
    };
    
    let target_astronomical_rate_sps = base_sidereal_rate_sps * astronomical_factor;

    // 3. Get the real-time SYSTEMIC correction factor from the PPS module.
    let pps_correction_factor = get_clock_correction_factor();
    let calibrated_rate = target_astronomical_rate_sps * self.tracking_calibration_factor;

    // 4. Apply the systemic correction to get the FINAL rate.
    let final_corrected_magnitude = calibrated_rate * pps_correction_factor;

    // Sidereal tracking must move the RA axis in the negative direction.
    let final_tracking_vector = [ final_corrected_magnitude, 0.0, 0.0 ];

    // --- Logging for Diagnostics ---
    info!("--- Tracking Rate Calculation ---");
    info!("Base Sidereal Rate: {:.4} steps/sec", base_sidereal_rate_sps);
    info!("Target Mode:        {:?}", tracking_mode);
    info!("Astronomical Factor:{:.6}", astronomical_factor);
    info!("Target Astro Rate:  {:.4} steps/sec", target_astronomical_rate_sps);
    info!("PPS System Factor:  {:.8}", pps_correction_factor);
    info!("Tracking calibration factor: {:.8}", self.tracking_calibration_factor);
    info!("FINAL Corrected Rate: {:.4} steps/sec", final_tracking_vector[0]);
    info!("---------------------------------");
    
    // Drop the lock before calling the planner to avoid holding it across a potential block.
    drop(app_state);

    //if let Ok(mut planner) = self.system.planner.lock() {
    //    if let Err(e) = planner.track_rate_sps(final_tracking_vector) {
    //        error!("Failed to start tracking: {}", e);
    //        // If it fails, update the state to reflect that we are NOT tracking.
    //        self.app_state.lock().unwrap().is_tracking = false;
    //        return Err(anyhow::anyhow!("Planner was busy"));
    //    }
    //}
    {
        let _cs = CriticalSection::new();
        let planner = unsafe { &mut *self.system.planner.0 };
        if let Err(e) = planner.track_rate_sps(final_tracking_vector) {
            // If the command fails, revert the state change.
            self.app_state.lock().unwrap().is_tracking = false;
            return Err(anyhow::anyhow!(e));
        }
    }
    Ok(())
    }

    pub fn execute_set_tracking(&self, enabled: bool, rate: TrackingRateType) -> Result<()> {
        if !enabled {
            info!("Stopping tracking via execute_set_tracking.");
            let _cs = CriticalSection::new();
            let planner = unsafe { &mut *self.system.planner.0 };
            planner.stop(); // Use the simplest stop method.
            self.app_state.lock().unwrap().is_tracking = false;
            return Ok(());
        }
    
        // If the command is to enable tracking, we run the start logic,
        // but we force the app_state to use the rate provided in the command.
        info!("Executing Set Tracking command for rate: {:?}", rate);
        let mut app_state = self.app_state.lock().unwrap();
        let tracking_mode: TrackingRateType = app_state.tracking_rate.into();

        if rate != TrackingRateType::Sidereal {
            // If the rate is not Sidereal, we need to record the time.
            app_state.last_non_sidereal_track_time = Some(Epoch::now()?);
        } else {
            // If the rate is Sidereal, we reset the last_non_sidereal_track_time.
            app_state.last_non_sidereal_track_time = None;
        }
        // Force the app_state to the desired rate for this operation.
        app_state.tracking_rate = rate as i32;
        app_state.is_tracking = true;
    
        // The rest of the logic is the same as execute_start_tracking
        let base_sidereal_rate_sps = self.ra_drivetrain.get_sidereal_tracking_rate_sps();
        let astronomical_factor = match rate {
            TrackingRateType::Solar => 23.9344699 / 24.0,
            TrackingRateType::Lunar => 0.966,
            TrackingRateType::King => 86164.09 / (86164.09 - 15.0),
            _ => 1.0,
        };
        let target_astronomical_rate_sps = base_sidereal_rate_sps * astronomical_factor;
        let pps_correction_factor = get_clock_correction_factor();
        let calibrated_rate = target_astronomical_rate_sps * self.tracking_calibration_factor;
        let final_corrected_sps = calibrated_rate * pps_correction_factor;

        // --- Logging for Diagnostics ---
        info!("--- Tracking Rate Calculation ---");
        info!("Base Sidereal Rate: {:.4} steps/sec", base_sidereal_rate_sps);
        info!("Target Mode:        {:?}", tracking_mode);
        info!("Astronomical Factor:{:.6}", astronomical_factor);
        info!("Target Astro Rate:  {:.4} steps/sec", target_astronomical_rate_sps);
        info!("PPS System Factor:  {:.8}", pps_correction_factor);
        info!("Tracking calibration factor: {:.8}", self.tracking_calibration_factor);
        info!("FINAL Corrected Rate: {:.4} steps/sec", final_corrected_sps);
        info!("---------------------------------");
    
        drop(app_state);

        // --- Command the Planner (Single Critical Section) ---
        {
            let _cs = CriticalSection::new();
            let planner = unsafe { &mut *self.system.planner.0 };
            if let Err(e) = planner.track_rate_sps([final_corrected_sps, 0.0, 0.0]) {
                // If the command fails, revert the state change.
                self.app_state.lock().unwrap().is_tracking = false;
                return Err(anyhow::anyhow!(e));
            }
        }
        Ok(())
    }

    // Stops Sidereal tracking.
    pub fn stop_tracking(&self) -> Result<()> {
        info!("Commanded to stop sidereal tracking...");
        
        let cmd = Command::StopTracking;
        self.send_command(cmd);
        Ok(())
    }
    pub fn execute_stop_tracking(&self) -> Result<()> {
        info!("Stopping sidereal tracking...");
        //self.app_state.lock().unwrap().tracking_enabled = false;
        //if let Ok(mut planner) = self.system.planner.lock() {
        //    planner.stop();
        //    self.app_state.lock().unwrap().is_tracking = false;
        //}
        
        info!("Sidereal tracking stopped.");
        //Ok(())
        self.execute_set_tracking(false, TrackingRateType::Sidereal)
    }

    // --- Guiding ---
    /// Issues a command to the commander task to perform a pulse guide.
    /// This method is non-blocking and returns immediately.
    pub fn command_pulse_guide(&self, direction: GuideDirection, duration_ms: u32) -> Result<()> {
        info!("Pulse guide command issued: {:?} for {}ms", direction, duration_ms);
        let cmd = Command::PulseGuide { direction, duration_ms };
        self.send_command(cmd);
        Ok(())
    }

    /// Executes the pulse guide. This is a blocking operation *within the commander task*.
    /// It calculates the combined tracking and guiding speeds, commands the move,
    /// waits for the duration, and then restores the original tracking speed.
    pub fn execute_pulse_guide(&self, direction: GuideDirection, duration_ms: u32) -> Result<(), anyhow::Error> {
        info!("Executing PulseGuide: {:?} for {}ms", direction, duration_ms);

        // --- 1. Get ALL state and configuration in one scope ---
        let (was_tracking, guide_ra_dps, guide_dec_dps) = {
            let app_state = self.app_state.lock().unwrap();
            (
                app_state.is_tracking,
                app_state.guide_ra_rate,  // This is in degrees per second from the API
                app_state.guide_dec_rate, // This is in degrees per second from the API
            )
        }; // Lock is released here.
    
        // --- 2. Calculate rates based on the pre-pulse state and configuration ---
        let (initial_rates, pulse_rates) = {
            let sidereal_rate_sps = self.ra_drivetrain.get_sidereal_tracking_rate_sps();
        
            let pre_pulse_rates = if was_tracking {
                [sidereal_rate_sps, 0.0, 0.0]
            } else {
                [0.0, 0.0, 0.0]
            };

            // --- THIS IS THE KEY CHANGE ---
            // Convert the configured guide rates from degrees/sec to steps/sec.
            // We use the `degrees_to_steps_relative` function which correctly converts a
            // distance (degrees) into a distance (steps) without applying offsets.
            let guide_ra_sps = self.ra_drivetrain.degrees_to_steps_relative(guide_ra_dps as f64) as f64;
            let guide_dec_sps = self.dec_drivetrain.degrees_to_steps_relative(guide_dec_dps as f64) as f64;

            let guide_vector = match direction {
                // Note: West is positive because it ADDS to the existing westward tracking motion.
                // East is negative because it SUBTRACTS from it.
                GuideDirection::West  => [guide_ra_sps, 0.0, 0.0],
                GuideDirection::East  => [-guide_ra_sps, 0.0, 0.0],
                GuideDirection::North => [0.0, guide_dec_sps, 0.0],
                GuideDirection::South => [0.0, -guide_dec_sps, 0.0],
            };

            let combined_rates = [
                pre_pulse_rates[0] + guide_vector[0],
                pre_pulse_rates[1] + guide_vector[1],
                pre_pulse_rates[2] + guide_vector[2],
            ];
        
            // Log for diagnostics
            info!("--- Guide Rate Calculation ---");
            info!("Configured Guide Rates (deg/s): RA {:.4}, Dec {:.4}", guide_ra_dps, guide_dec_dps);
            info!("Calculated Guide Rates (steps/s): RA {:.4}, Dec {:.4}", guide_ra_sps, guide_dec_sps);
            info!("Final Pulse Rates (steps/s): {:?}", combined_rates);
            info!("------------------------------");

            (pre_pulse_rates, combined_rates)
        };
    
        // --- 3. Execute the Pulse (unchanged) ---
        self.app_state.lock().unwrap().is_guiding = true;
        //let _ = self.system.planner.lock().unwrap().guide_rate_sps(pulse_rates);
        {
            let _cs = CriticalSection::new();
            let planner = unsafe { &mut *self.system.planner.0 };
            let _ = planner.guide_rate_sps(pulse_rates);
        }

        FreeRtos::delay_ms(duration_ms);

        // --- 4. Restore the EXACT state from before the pulse (unchanged) ---
        //let _ = self.system.planner.lock().unwrap().track_rate_sps(initial_rates);
        {
            let _cs = CriticalSection::new();
            let planner = unsafe { &mut *self.system.planner.0 };
            let _ = planner.track_rate_sps(initial_rates);
        }
        self.app_state.lock().unwrap().is_guiding = false;
    
        info!("PulseGuide finished. Returned to previous state (Tracking: {}).", was_tracking);
        Ok(())
    }

    // --- Syncing --- 
    pub fn command_sync_to_coordinates(&self, ra: RightAscension, dec: Declination) -> Result<()> {
        let cmd = Command::SyncToCoordinates { ra, dec };
        self.send_command(cmd);
        Ok(())
    }

    pub fn execute_sync(&self, sync_ra: RightAscension, sync_dec: Declination) -> Result<()> {
        info!("Executing Sync to RA: {}, DEC: {}", sync_ra.as_hms_string(), sync_dec.as_dms_string());

        // Get the current LST to calculate what the physical position SHOULD be.
        let current_lst = self.system.time.get_lst();

        // Calculate the absolute step position that corresponds to the sync coordinates.
        let target_ra_steps = sync_ra.as_motor_steps(current_lst, &self.ra_drivetrain);
        let target_dec_steps = sync_dec.as_motor_steps(&self.dec_drivetrain);
        
        // Tell the planner: "Your current physical position should from now on be
        // considered this logical step position."
        // This updates the internal offset.
        //self.system.planner.lock().unwrap().sync_to_position([target_ra_steps, target_dec_steps, 0]);
        {
            let _cs = CriticalSection::new();
            let planner = unsafe { &mut *self.system.planner.0 };
            planner.sync_to_position([target_ra_steps, target_dec_steps, 0]);
        }
        
        info!("Sync complete.");
        Ok(())
    }
}