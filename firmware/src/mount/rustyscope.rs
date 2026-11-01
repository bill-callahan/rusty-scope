// src/mount/rustyscope.rs

// use super::app_state::SharedAppState;
use crate::mount::app_state::{PreFocusState, SharedAppState};

use crate::mount::{Latitude, Longitude};
use crate::storage::DeviceConfig;
use crate::systemsvcs::{SendWrapper, SystemServices}; // It depends on the System struct
use crate::mount::{
    AxisDrivetrain,
    rightascension::RightAscension,
    declination::Declination,
    site::Site,
    commands::Command,
};
use anyhow::Result;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::task::CriticalSection;
use esp_idf_svc::sys::{uxTaskGetStackHighWaterMark, xQueueGenericCreate, xQueueGenericSend, xQueueReceive, xTaskGetCurrentTaskHandle, QueueHandle_t};
use hifitime::Epoch;
use log::{info, error};
use std::sync::{Arc, Mutex};
use crate::stepper::PlannerMode;


/// The main astronomical controller for the telescope.
/// This struct contains all the high-level logic for GoTo, Sync, and Tracking.
pub struct RustyScope<'a> {
    // It holds references to the services it needs from the System struct.
    pub system: &'a SystemServices<'a>,
    pub app_state: SharedAppState,
    pub command_queue: SendWrapper<QueueHandle_t>,
    pub device_config: Arc<Mutex<DeviceConfig>>,
    
    // It owns the configuration for the mount's physical properties.
    pub ra_drivetrain: AxisDrivetrain,
    pub dec_drivetrain: AxisDrivetrain,
    pub focus_drivetrain: AxisDrivetrain,
    pub tracking_calibration_factor: f64,
}

impl<'a> RustyScope<'a> {
    /// Creates a new instance of the telescope controller.
    pub fn new(system: &'a SystemServices<'a>, app_state: SharedAppState) -> Result<Self> {
        info!("Initializing RustyScope astronomical controller...");
        let mut data = app_state.lock().unwrap();
        
        // --- where are we? GPS will tell us - 
        let gps = system.gps.get_data();
        let lon = Longitude::new(gps.longitude);
        let lat = Latitude::new(gps.latitude);
        let site = Site::new("GPS".to_string(), lat, lon, gps.elevation);
        site.print_summary();
        data.site_info = site;
        
        
        // --- Load machine configuration from SPIFFS---
        let spiffs = system.spiffs.as_ref().ok_or_else(|| anyhow::anyhow!("SPIFFS storage not initialized"))?;
        let mut machineconfig = if let Ok(config) = spiffs.load_config() {
            if config.configured {
                info!("Using existing configuration");
                config
            } else {
                info!("Configuration not configured. Creating new config.");
                spiffs.set_config()?
            }
        } else {
            info!("Failed to load config. Creating new config.");
            spiffs.new_config()?
        };
        let device_config = Arc::new(Mutex::new(machineconfig));

        // --- Drivetrain Configuration ---
        let offset_degrees = if lat.as_degrees() < 0.0 {
            info!("Setting origin declination to -90.0");
            -90.0
        } else {
            info!("Setting origin declination to 90.0");
            90.0
        };
        machineconfig.motory_offset = offset_degrees;
        machineconfig.motorx_gratio = 99.0505590666 * (58.0/16.0); // Planetary gear (99-104/2506) * (gear output)
        machineconfig.motory_gratio = 99.0505590666 * (54.0/16.0);
        
        let ra_drivetrain: AxisDrivetrain = AxisDrivetrain::new(
            machineconfig.motorx_spr, 
            machineconfig.motorx_microsteps, 
            machineconfig.motorx_gratio, 
            machineconfig.motorx_offset);
        let dec_drivetrain: AxisDrivetrain = AxisDrivetrain::new(
            machineconfig.motory_spr, 
            machineconfig.motory_microsteps, 
            machineconfig.motory_gratio, 
            machineconfig.motory_offset);
        let focus_drivetrain: AxisDrivetrain = AxisDrivetrain::new(
            machineconfig.motorz_spr, 
            machineconfig.motorz_microsteps, 
            machineconfig.motorz_gratio, 
            machineconfig.motorz_offset);
        
        // You could print the summary here if you want
        print!("\nRA Drivetrain Summary:");
        ra_drivetrain.print_summary();
        println!("Calibration offset: {}", machineconfig.tracking_correction_factor);
        print!("\nDEC Drivetrain Summary:");
        dec_drivetrain.print_summary();
        print!("\nFocus Drivetrain Summary:");
        focus_drivetrain.print_summary();

        // Populate the live AppState with the persistent guide rate configuration.
        info!(
            "Loading guide rates from config: RA={:.4} dps, Dec={:.4} dps",
            machineconfig.guide_ra_rate_dps, machineconfig.guide_dec_rate_dps
        );
        data.guide_ra_rate = machineconfig.guide_ra_rate_dps;
        data.guide_dec_rate = machineconfig.guide_dec_rate_dps;

        info!(
            "Loading default rates from config: Tracking={}, MoveRA={:.2} dps, MoveDec={:.2} dps",
            machineconfig.default_tracking_rate,
            machineconfig.move_axis_ra_rate_dps,
            machineconfig.move_axis_dec_rate_dps
        );
        data.tracking_rate = machineconfig.default_tracking_rate;
        data.ra_rate = machineconfig.move_axis_ra_rate_dps;
        data.dec_rate = machineconfig.move_axis_dec_rate_dps;
        //let tracking_calibration_factor = machineconfig.tracking_calibration_factor;
        let tracking_calibration_factor = 1.005;

        // Setting some defaults values
        data.current_ra = RightAscension::new_from_hours(system.time.get_lst());
        data.current_dec = Declination::new(offset_degrees);
        data.target_ra = RightAscension::new_from_hours(system.time.get_lst());
        data.target_dec = Declination::new(offset_degrees);
        //(data.current_alt, data.current_az) = Self::equatorial_to_horizontal(&data.current_ra, &data.current_dec, system.time.get_lst(), lat.as_degrees());

        // Setup the command controller queue
        let command_queue = unsafe {
            xQueueGenericCreate(10, std::mem::size_of::<Command>() as u32, 0)
        };
        



        




        // lastly, save the updated `config` struct, but handle the error gracefully.
        info!("[RustyScope] Attempting to save configuration to SPIFFS...");
        match system.spiffs.as_ref().unwrap().save_config(&machineconfig) {
            Ok(_) => {
                info!("Final state saved successfully to SPIFFS!");
            }
            Err(e) => {
                // If it fails, print a detailed error message instead of just exiting.
                error!("!!! FAILED to save configuration to SPIFFS: {:?} !!!", e);
            }
        }
        drop(data);

        Ok(Self {
            system,
            app_state,
            device_config,
            ra_drivetrain,
            dec_drivetrain,
            focus_drivetrain,
            tracking_calibration_factor,
            command_queue: SendWrapper(command_queue),
        })
    }

    pub fn get_tracking_correction_factor(&self) -> f64 {
        self.device_config.lock().unwrap().tracking_correction_factor
    }

    

    /// Stops all motion.
    pub fn execute_stop_motion(&self) {
        info!("Stopping all motion.");
    
        // 1. Create a critical section. This disables interrupts for the duration of the block.
        let _cs = CriticalSection::new();
    
        // 2. Safely get a mutable reference to the planner from the raw pointer.
        //    The `_cs` guard variable ensures this is safe.
        let planner = unsafe { &mut *self.system.planner.0 };

        // 3. Call the method on the planner.
        planner.stop();

        // 4. Update the app_state (this uses a separate Mutex, which is fine).
        self.app_state.lock().unwrap().is_tracking = false;

        // The critical section ends when `_cs` goes out of scope, and interrupts are re-enabled.
    }

    /// Checks if the mount is currently executing a finite slew.
    pub fn is_slewing(&self) -> bool {
        // 1. Create a critical section.
        let _cs = CriticalSection::new();

        // 2. Safely get an *immutable* reference to the planner.
        //    We can use `& *` instead of `&mut *` for read-only access.
        let planner = unsafe { &*self.system.planner.0 };

        // 3. Call the method and return the value.
        planner.is_slewing()
    
        // The critical section ends here.
    }

    /// Checks if the mount is in any non-idle state (slewing, tracking, or guiding).
    pub fn is_busy(&self) -> bool {
        // This pattern is identical to is_slewing().
        let _cs = CriticalSection::new();
        let planner = unsafe { &*self.system.planner.0 };
        planner.is_busy()
    }

    pub fn command_park(&self) -> Result<()> {
        info!("Park command issued.");
        self.send_command(Command::Park);
        Ok(())
    }
    
    pub fn command_home(&self) -> Result<()> {
        info!("Home command issued.");
        self.send_command(Command::Home);
        Ok(())
    }

    /// Unparking is a direct, non-blocking state change. It does not need to go
    /// through the commander's queue.
    pub fn command_unpark(&self) -> Result<(), &'static str> {
        let mut state = self.app_state.lock().unwrap();
        if !state.is_parked {
            return Err("Mount is already unparked.");
        }
        
        info!("Unparking mount.");
        state.is_parked = false;
        state.at_park = false; // The mount is no longer considered 'at park'
        
        // Optional: If you want the mount to start tracking immediately after unparking.
        // This is a common feature.
        if state.tracking_enabled {
            info!("Resuming tracking after unpark.");
            // We can directly call execute_start_tracking here or send a command.
            // Sending a command is safer to avoid potential deadlocks if this method grows.
            self.send_command(Command::StartTracking);
        }
        
        Ok(())
    }

    // --- Internal Execution Methods (called by commander_task) ---

    // Your implementation is good. The park position is configurable via the drivetrain's home offset.
    // Park is often defined as HA=0, Dec=pole angle, or something similar depending on mount type.
    // For a GEM, a common park position is counter-weight down. Let's define this as
    // RA at home position (0 steps) and Dec at horizon (0 degrees).
    pub fn execute_park(&self) -> Result<()> {
        info!("Executing Park command...");
        // 1. Stop any current tracking or slewing.
        self.execute_stop_motion();
        
        // 2. Define the park position in motor steps.
        // RA axis at its home position (0 steps).
        let park_ra_steps = 0;
        // DEC axis pointing at the horizon (0 degrees). We use the drivetrain to convert.
        let park_dec_steps = self.dec_drivetrain.degrees_to_steps(0.0);
        let park_pos = [park_ra_steps, park_dec_steps, 0];
        info!("Calculated park position (steps): {:?}", park_pos);

        // 3. Command the planner to move to the park position.
        // let _ = self.system.planner.lock().unwrap().move_to_steps(park_pos);
        let _cs = CriticalSection::new();
        let planner = unsafe { &mut *self.system.planner.0 };
        let _ = planner.move_to_steps(park_pos);
        
        // Note: The `is_parked` and `at_park` flags will be set by the commander
        // task *after* the slew completes.
        Ok(())
    }

    pub fn execute_home(&self) -> Result<()> {
        info!("Executing Home command for RA and DEC axes...");
    
        // 1. Stop any current motion first.
        self.execute_stop_motion();
    
        // 2. Lock the planner to get the focuser's current absolute position.
        //    We need to keep the lock for the whole operation to prevent a race condition
        //    where the focuser position could change between reading it and commanding the move.
        // let mut planner = self.system.planner.lock().unwrap();
        let _cs = CriticalSection::new();
        let planner = unsafe { &mut *self.system.planner.0 };
    
        // 3. Get the focuser's (Axis Z) current position from the planner.
        let current_focuser_steps = planner.current_position_steps()[2];
    
        // 4. Define the target position.
        //    - RA (Axis X) goes to its physical zero.
        //    - DEC (Axis Y) goes to its physical zero.
        //    - Focuser (Axis Z) is commanded to move to its *current* position, so it doesn't move at all.
        let home_position = [0, 0, current_focuser_steps];
    
        info!("Targeting RA=0, DEC=0, while keeping Focuser at {} steps.", current_focuser_steps);

        // 5. Command the planner to move to the new target position.
        //    Since we still hold the planner lock, we call the method directly on `planner`.
        let _ = planner.move_to_steps(home_position);
    
        // The `at_home` flag will be set by the commander task after the move completes.
    
        Ok(())
    }

    /// Helper function to send command
    pub fn send_command(&self, cmd: Command) {
       let result = unsafe {
            xQueueGenericSend(self.command_queue.0, &cmd as *const _ as *mut _, 0, 0)
        };
        if result != 1 {
            error!("[send_command] Failed to send command to queue: {}", result);
        }
    }

    /// Loads the full, persistent `DeviceConfig` struct from the SPIFFS filesystem.
    ///
    /// This method provides a clean, centralized way for any part of the application
    /// (like the web UI backend) to access the saved configuration.
    /// It handles the case where the SPIFFS service might not be available.
    pub fn get_persistent_config(&self) -> Result<DeviceConfig> {
        // Get a handle to the storage manager.
        // The `ok_or_else` provides a clear error message if SPIFFS is not initialized.
        let storage = self.system.spiffs.as_ref()
            .ok_or_else(|| anyhow::anyhow!("SPIFFS storage service not available."))?;

        // Delegate the actual loading logic to the storage manager.
        storage.load_config()
    }

    /// Saves a modified `DeviceConfig` struct back to the SPIFFS filesystem.
    ///
    /// This method is the counterpart to `get_persistent_config` and is used
    /// by the configuration web UI to save user changes.
    pub fn save_persistent_config(&self, config: &DeviceConfig) -> Result<()> {
        // Get a handle to the storage manager.
        let storage = self.system.spiffs.as_ref()
            .ok_or_else(|| anyhow::anyhow!("SPIFFS storage service not available."))?;

        // Delegate the saving logic to the storage manager.
        storage.save_config(config)
    }

    /// Deletes the persistent `config.json` file from the SPIFFS filesystem.
    /// This is used for a factory reset.
    pub fn delete_persistent_config(&self) -> Result<()> {
        let storage = self.system.spiffs.as_ref()
            .ok_or_else(|| anyhow::anyhow!("SPIFFS storage service not available."))?;
        
        // We need to add a `delete_config` method to `StorageManager`.
        storage.delete_config()
    }

}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CommanderState {
    Idle,
    WaitingForCoarseSlew,
    WaitingForFineSlew,
    WaitingForFocus,
    WaitingForPark,
    WaitingForHome,
    CalibratingWait,
    CalibratingReslew,
}

struct CalibrationData {
    start_time: f64,  // UNIX timestamp
    start_ra_steps: i32,
}


// --- This is the entry point for our Commander task ---
pub extern "C" fn commander_task_entry(arg: *mut core::ffi::c_void) {
    // 1. Safely reconstitute the Arc from the raw pointer.
    // This is safe because we leaked the original Box in `main`.
    let scope: Arc<RustyScope> = unsafe { Arc::from_raw(arg as *const RustyScope) };
    
    // Now we are in a normal Rust context within this new task.
    info!("Commander task started on core {:?}.", esp_idf_svc::hal::cpu::core());
    let commander_task_handle = unsafe { xTaskGetCurrentTaskHandle() };
    let mut loop_counter = 0;
    let mut current_cmd  = Some(Command::None);
    let mut commander_state = CommanderState::Idle;
    let mut calibration_data: Option<CalibrationData> = None;

    // --- Main Commander loop ---
    loop {
        // --- Part 1: Handle External Commands (Non-Blocking Receive) ---
        let mut new_cmd = Command::None;
        // The timeout is now 0, so this call returns instantly if the queue is empty.
        if unsafe { xQueueReceive(scope.command_queue.0, &mut new_cmd as *mut _ as *mut _, 0) } == 1 {
            // We successfully received a new command from the outside world.
            info!("Commander task received new command: {:?}", new_cmd);

            // High-priority commands that interrupt anything.
            if let Command::StopMotion = new_cmd {
                scope.execute_stop_motion();
                current_cmd = None; // Cancel any pending command
                commander_state = CommanderState::Idle; // force state back to Idle
                continue;           // restart the loop immediately.
            }
            // For any other command, check if we need to override.
            let is_planner_slewing = {
                let _cs = CriticalSection::new();
                let planner = unsafe { &*scope.system.planner.0 };
                planner.is_slewing()
            };

            if is_planner_slewing {
                info!("Planner is busy with a slew, but new command received. Overriding.");
                scope.execute_stop_motion();
                // CRITICAL FIX: Reset the commander's state machine to Idle.
                commander_state = CommanderState::Idle;
            }
            // Set the new command as our current goal.
            current_cmd = Some(new_cmd);
        }

        // --- Part 2: Execute State Machine Logic ---
        // This is where we decide what to do based on the command.
        // Is the planner idle and do we have a command to execute?
        let is_planner_slewing = {
            let _cs = CriticalSection::new();
            let planner = unsafe { &*scope.system.planner.0 };
            planner.is_slewing()
        };
        
        match commander_state {
            // =========================
            //  IDLE State Handler
            // =========================
            CommanderState::Idle => {
                
                if let Some(cmd_to_run) = current_cmd.take() {
                    
                    // --- PRE-COMMAND PREPARATION LOGIC ---
                    // Before executing a command that starts a *finite* move,
                    // we must ensure any *continuous* move (like tracking) is stopped.
                    match cmd_to_run {
                        Command::SlewToTarget | Command::Park | Command::Home | Command::CalibrateTracking => {
                            if scope.app_state.lock().unwrap().is_tracking {
                                info!("Stopping tracking before starting new finite move command.");
                                if let Err(e) = scope.execute_stop_tracking() {
                                     error!("Error stopping tracking before move: {}", e);
                                }
                            }
                        }
                        _ => { /* Other commands don't need this prelude */ }
                    }

                    info!("Executing command from Idle state: {:?}", cmd_to_run);
                    
                    // --- COMMAND DISPATCH LOGIC ---
                    match cmd_to_run {
                        // --- Finite Move Commands ---
                        Command::SlewToTarget => {
                            scope.app_state.lock().unwrap().tracking_enabled = true;
                            if scope.app_state.lock().unwrap().is_parked {
                                error!("Cannot slew: Mount is parked. Please unpark first.");
                            } else if scope.execute_slew_to_target().is_ok() {
                                commander_state = CommanderState::WaitingForCoarseSlew;
                            }
                        }
                        Command::MoveAxis { axis, rate_dps } => {
                            if is_planner_slewing {
                                error!("Cannot MoveAxis, planner is busy slewing.");
                            } else if scope.app_state.lock().unwrap().is_parked {
                                error!("Cannot MoveAxis, mount is parked.");
                            } else if let Err(e) = scope.execute_move_axis(axis, rate_dps) {
                                error!("Failed to execute MoveAxis: {}", e);
                            }
                        }
                        Command::Park => {
                            if scope.execute_park().is_ok() {
                                commander_state = CommanderState::WaitingForPark;
                            }
                        }
                        Command::Home => {
                            if scope.execute_home().is_ok() {
                                commander_state = CommanderState::WaitingForHome;
                            }
                        }
                        Command::MoveFocuser { position } => {
                             // --- This is the start of your focus logic ---
                            info!("Starting focus move. Pausing other activities if necessary.");
                
                            // a) Get current mount state (is it tracking?)
                            let was_tracking = scope.app_state.lock().unwrap().is_tracking;
                
                            // b) Store pre-focus state so we can resume later.
                            scope.app_state.lock().unwrap().pre_focus_state = if was_tracking {
                                PreFocusState::WasTracking
                            } else {
                                PreFocusState::Idle
                            };
                
                            // c) Stop current motion (e.g., tracking) to dedicate the planner to focusing.
                            if was_tracking {
                                if let Err(e) = scope.execute_stop_tracking() {
                                    error!("Failed to stop tracking for focus move: {}", e);
                                }
                            }
                
                            // d) Command the absolute focus move.
                            if scope.execute_move_focuser(position).is_ok() {
                                // e) Transition the commander's state to wait for completion.
                                commander_state = CommanderState::WaitingForFocus;
                            }
                        }

                        // --- Continuous Move Commands ---
                        Command::StartTracking => {
                            if scope.app_state.lock().unwrap().is_parked {
                                error!("Cannot start tracking: Mount is parked.");
                            } else if let Err(e) = scope.execute_start_tracking() {
                                error!("Failed to start tracking: {}", e);
                            }
                        }
                        Command::StopTracking => {
                            if let Err(e) = scope.execute_stop_tracking() {
                                 error!("Failed to stop tracking: {}", e);
                            }
                        }
                        Command::SetTracking { enabled, rate } => {
                            if let Err(e) = scope.execute_set_tracking(enabled, rate) {
                                error!("Failed to set tracking state: {}", e);
                            }
                        }

                        // --- Timed/Blocking "Self-Contained" Commands ---
                        Command::PulseGuide { direction, duration_ms } => {
                            if is_planner_slewing {
                                error!("Cannot pulse guide: mount is busy slewing.");
                            } else if scope.app_state.lock().unwrap().is_parked {
                                error!("Cannot pulse guide: mount is parked.");
                            } else if let Err(e) = scope.execute_pulse_guide(direction, duration_ms) {
                                error!("Pulse guide execution failed: {}", e);
                            }
                        }
                        
                        // --- Other Commands ---
                        Command::SyncToCoordinates { ra, dec } => {
                            if let Err(e) = scope.execute_sync(ra, dec) {
                                error!("Failed to sync: {}", e);
                            }
                        }
                        Command::CalibrateTracking => {
                            // --- START CALIBRATION ---
                            info!("[CALIBRATE] Starting tracking calibration routine.");
                            info!("[CALIBRATE] Ensure you are pointed at a star near the celestial equator.");

                            let _cs = CriticalSection::new();
                            let planner = unsafe { &mut *scope.system.planner.0 };
        
                            // 1. Get current state
                            let start_ra_steps = planner.current_position_steps()[0];
                            let start_time = Epoch::now().unwrap().to_unix_seconds();
                            calibration_data = Some(CalibrationData { start_time, start_ra_steps });
        
                            // 2. Start tracking at the UNCORRECTED rate
                            let ideal_rate = scope.ra_drivetrain.get_sidereal_tracking_rate_sps();
                            let corr = scope.get_tracking_correction_factor();
                            info!("[CALIBRATE] Corrected rate - {}", ideal_rate * corr);
                            // scope.system.planner.lock().unwrap().track_rate_sps([ideal_rate * corr, 0.0, 0.0]).unwrap();
                            planner.track_rate_sps([ideal_rate * corr, 0.0, 0.0]).unwrap();
                            scope.app_state.lock().unwrap().is_tracking = true;
        
                            info!("[CALIBRATE] Now tracking at ideal rate. Please wait 5 minutes...");
                            commander_state = CommanderState::CalibratingWait;
                        }

                        // Ignore commands that don't have an execution part here.
                        Command::UpdateTelemetry | Command::StopMotion | Command::None => {}
                    }
                }
            }

            // ===================================
            //  WAITING State Handlers
            // ===================================
            CommanderState::WaitingForCoarseSlew => {
                // We MUST take the snapshot HERE, inside the state we are checking.
                let snapshot = {
                    let _cs = CriticalSection::new();
                    let planner = unsafe { &*scope.system.planner.0 };
                    //planner.is_slewing()
                    planner.get_snapshot()
                    
                };

                // --- NEW LOGGING ---
                if loop_counter % 10 == 0 { // Log every second
                    info!("[SLEW_MONITOR] Current Pos: {:?}", snapshot.1);
                }

                //if !is_planner_slewing {
                if snapshot.0 != PlannerMode::Moving {
                    info!("Coarse slew complete. Performing correction slew.");
                    if scope.execute_slew_to_target().is_ok() {
                        commander_state = CommanderState::WaitingForFineSlew;
                    } else {
                        commander_state = CommanderState::Idle;
                    }
                }
            }
            CommanderState::WaitingForFineSlew => {
                // We MUST take the snapshot HERE, inside the state we are checking.
                let is_planner_slewing = {
                    let _cs = CriticalSection::new();
                    let planner = unsafe { &*scope.system.planner.0 };
                    planner.is_slewing()
                };

                if !is_planner_slewing {
                    info!("Fine tune slew complete. Slew complete.");
                    
                    // Get a lock on the application state.
                    let app_state = scope.app_state.lock().unwrap();
                    
                    // After a successful slew, we should always start tracking,
                    // UNLESS the user has explicitly parked the mount in the meantime.
                    if !app_state.is_parked {
                        // We also check the 'master switch' for tracking. If the user
                        // sent a "tracking off" command *during* the slew, we should respect that.
                        if app_state.tracking_enabled {
                            info!("Automatically starting tracking after slew.");
                            // Drop the lock before calling the function to avoid deadlocks
                            drop(app_state); 
                            
                            if let Err(e) = scope.execute_start_tracking() {
                                error!("Failed to start tracking after slew: {}", e);
                            }
                        } else {
                            info!("Slew complete, but tracking is disabled by user. Remaining idle.");
                        }
                    } else {
                        info!("Slew complete, but mount is parked. Not starting tracking.");
                    }
                    
                    // Transition back to Idle regardless.
                    commander_state = CommanderState::Idle;
                }
            }
            CommanderState::WaitingForPark => {
                // We MUST take the snapshot HERE, inside the state we are checking.
                let is_planner_slewing = {
                    let _cs = CriticalSection::new();
                    let planner = unsafe { &*scope.system.planner.0 };
                    planner.is_slewing()
                };

                if !is_planner_slewing {
                    info!("Park move complete.");
                    let mut state = scope.app_state.lock().unwrap();
                    state.is_parked = true;
                    state.at_park = true;
                    commander_state = CommanderState::Idle;
                }
            }
            CommanderState::WaitingForHome => {
                // We MUST take the snapshot HERE, inside the state we are checking.
                let is_planner_slewing = {
                    let _cs = CriticalSection::new();
                    let planner = unsafe { &*scope.system.planner.0 };
                    planner.is_slewing()
                };

                if !is_planner_slewing {
                    info!("Home move complete.");
                    scope.app_state.lock().unwrap().at_home = true;
                    commander_state = CommanderState::Idle;
                }
            }
            CommanderState::WaitingForFocus => {
                // We MUST take the snapshot HERE, inside the state we are checking.
                let is_planner_slewing = {
                    let _cs = CriticalSection::new();
                    let planner = unsafe { &*scope.system.planner.0 };
                    planner.is_slewing()
                };

                if !is_planner_slewing {
                    info!("Focus move complete. Resuming previous state.");
                    
                    // a) Get the state we were in before focusing.
                    let resume_state = scope.app_state.lock().unwrap().pre_focus_state;
                    
                    // b) Resume tracking if we were tracking before.
                    if resume_state == PreFocusState::WasTracking {
                        if scope.app_state.lock().unwrap().tracking_enabled {
                            info!("Resuming tracking after focus move.");
                            if let Err(e) = scope.execute_start_tracking() {
                                error!("Failed to resume tracking: {}", e);
                            }
                        }
                    }
                    
                    // c) Transition the commander back to idle. THIS IS CRUCIAL.
                    commander_state = CommanderState::Idle;
                }
            }
            CommanderState::CalibratingWait => {
                // We stay in this state for a fixed duration.
                let data = calibration_data.as_ref().unwrap();
                let elapsed = Epoch::now().unwrap().to_unix_seconds() - data.start_time;

                if elapsed >= 300.0 { // 5 minutes
                    info!("[CALIBRATE] 5 minutes elapsed. Stopping track and re-slewing to target to measure error.");
                    scope.execute_stop_tracking().unwrap();
                    scope.execute_slew_to_target().unwrap(); // Slew back to the same star
                    commander_state = CommanderState::CalibratingReslew;
                }
            }
            CommanderState::CalibratingReslew => {
                // We MUST take the snapshot HERE, inside the state we are checking.
                let is_planner_slewing = {
                    let _cs = CriticalSection::new();
                    let planner = unsafe { &*scope.system.planner.0 };
                    planner.is_slewing()
                };

                if !is_planner_slewing {
                    info!("[CALIBRATE] Re-slew complete. Calculating correction factor.");
                    
                    
                    // --- Step 1: Atomic Snapshot of Planner Data ---
                    // Enter a critical section, get all the data we need from the planner,
                    // and then immediately exit the critical section.
                    let (final_ra_steps, error_steps) = {
                        let _cs = CriticalSection::new();
                        let planner = unsafe { &mut *scope.system.planner.0 };
                        let steps = planner.current_position_steps()[0];
                        let err = planner.slew_steps()[0];
                        planner.reset_slew_steps(); // Reset for next time
                        (steps, err)
                    }; // <-- Critical section ends HERE. Interrupts are now enabled.

                    let end_time = Epoch::now().unwrap().to_unix_seconds();
                    let data = calibration_data.take().unwrap();
                    
                    // --- THE MATH ---
                    let duration_secs = end_time - data.start_time;
                    let steps_tracked = final_ra_steps - data.start_ra_steps - error_steps;
                    
                    let measured_rate = steps_tracked as f64 / duration_secs;
                    let theory_rate = scope.ra_drivetrain.get_sidereal_tracking_rate_sps();
                    let ideal_rate = (steps_tracked + error_steps) as f64 / duration_secs;
                    
                    if measured_rate.abs() < 1e-6 {
                        error!("[CALIBRATE] Measured rate is zero. Aborting calibration.");
                        commander_state = CommanderState::Idle;
                        continue;
                    }
                    
                    //let correction_factor = ideal_rate / measured_rate;
                    let correction_factor = ideal_rate / theory_rate;
                    
                    info!("[CALIBRATE] --- CALCULATION RESULTS ---");
                    info!("[CALIBRATE] Duration: {:.2}s", duration_secs);
                    info!("[CALIBRATE] Steps Tracked: {}", steps_tracked);
                    info!("[CALIBRATE] Error Steps: {}", error_steps);
                    info!("[CALIBRATE] Theory rate: {}", theory_rate);
                    info!("[CALIBRATE] Measured Rate: {:.4} sps", measured_rate);
                    info!("[CALIBRATE] Ideal Rate:    {:.4} sps", ideal_rate);
                    info!("[CALIBRATE] New Correction Factor: {:.8}", correction_factor);
                    
                    // --- SAVE THE NEW FACTOR ---
                    {
                    let mut config = scope.device_config.lock().unwrap();
                    config.tracking_correction_factor = correction_factor;
                    scope.system.spiffs.as_ref().unwrap().save_config(&config).unwrap();
                    }
                    info!("[CALIBRATE] New factor saved to configuration. It will be used on next track.");
                    
                    
                    commander_state = CommanderState::Idle;
                }
            }
        }

        // --- Part 3: Periodic Telemetry Update (The "Telemetry Tick") ---
        // This runs regardless of motion state.
        //info!("Telemetry Tick");
        if let Err(e) = scope.execute_update_telemetry() {
            error!("Failed to update telemetry: {}", e);
        }

        loop_counter += 1;
        // Print stats every 300 iterations (e.g., every 30 seconds if loop delay is 100ms)
        if loop_counter >= 300 {
            let stack_hwm = unsafe { uxTaskGetStackHighWaterMark(commander_task_handle) };
            log::info!("[SYS_MONITOR] Commander Task - Stack HWM: {} bytes", stack_hwm);
            loop_counter = 0;
        }

        // --- Part 4: Delay ---
        // This loop runs at a moderate UI-level frequency, e.g., 10Hz
        FreeRtos::delay_ms(100);
            
        
    }
}

