// src/stepper/planner.rs

use std::sync::atomic::{AtomicU8, Ordering};

use crate::stepper::bitbang_driver::BitbangStepperDriver;
use crate::stepper::SpiStepperDriver;

use super::i2s0::I2sStepperDriver;
use super::sr74hc595::*;
use esp_idf_svc::sys::EspError;

// --- A global, atomic variable to hold the last "high" pulse byte ---
// We only need to see the high pulse, as the low pulse is always the same.
static LAST_STEP_BYTE: AtomicU8 = AtomicU8::new(0);

// --- STATE STRUCTS ---
// These structs hold the data for different types of moves.
// They must derive `Copy` to be used in the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct MoveState {
    target: [i32; 3],
    delta: [i32; 3],
    err: [u32; 2], // For Bresenham's algorithm
}


// --- THE C-STYLE STATE ENUM ---
// This enum has NO associated data. It's just a tag.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlannerMode {
    Idle,
    Moving,
    Tracking,
    Guiding,
}

#[derive(Clone, Copy, Debug, Default)]
struct Profile {
    max_speed: f32,
    acceleration: f32,
}

#[repr(C)]
pub struct MotionPlanner<'d> {
    //pub driver: I2sStepperDriver<'d>,
    pub driver: BitbangStepperDriver<'d>,
    //pub driver: SpiStepperDriver<'d>,
    pub current_pos: [i32; 3],

    // --- State is now stored directly in the struct ---
    pub mode: PlannerMode,
    move_data: MoveState, // Data for a finite move
    cont_numerators: [i128; 3],
    cont_directions: [i32; 3],
    
    rate_accumulators: [i128; 3],
    
    pub tracking_steps: [i32; 3],
    pub guiding_steps: [i32; 3],
    pub slew_steps: [i32; 3],
    pub sync_steps: [i32; 3],

    base_state: MotorState,
    update_frequency_hz: u64,
}

impl<'d> MotionPlanner<'d> {
    //pub fn new(driver: I2sStepperDriver<'d>, update_frequency_hz: u64) -> Self {
    pub fn new(driver: BitbangStepperDriver<'d>, update_frequency_hz: u64) -> Self {
    //pub fn new(driver: SpiStepperDriver<'d>, update_frequency_hz: u64) -> Self {
        Self {
            driver,
            current_pos: [0; 3],
            mode: PlannerMode::Idle,
            move_data: MoveState::default(),
            cont_numerators: [0; 3],
            cont_directions: [0; 3],
            rate_accumulators: [0; 3],
            tracking_steps: [0; 3],
            guiding_steps: [0; 3],
            slew_steps: [0; 3],
            sync_steps: [0; 3],
            base_state: MotorState::new().enable_motors(true),
            update_frequency_hz,
        }
    }

    // --- IRAM-Safe ISR Call Chain ---

    /// This is the main entry point called by the ISR. It MUST be in IRAM.
    #[inline(never)]
    #[link_section = ".iram1.text"]
    pub fn update(&mut self) -> Result<(), EspError> {
        // The match is on a simple C-style enum. This is guaranteed to be safe.
        
        match self.mode {
            PlannerMode::Idle => {}
            PlannerMode::Moving => {
                if self.current_pos == self.move_data.target {
                    self.mode = PlannerMode::Idle;
                } else {
                    self.update_finite_move()?;
                }
            }
            PlannerMode::Tracking | PlannerMode::Guiding => {
                self.update_continuous_move()?;
            }
        }
        Ok(())
    }

    /// Helper for finite moves. Called by `update`, so it MUST be in IRAM.
    /// It takes ownership of the state and returns the new state.
    #[inline(never)]
    #[link_section = ".iram1.text"]
    fn update_finite_move(&mut self) -> Result<(), EspError> {
        let move_state = &mut self.move_data;

        let mut step_high_state = self.base_state;
        let dx = move_state.delta[0].wrapping_abs() as u32;
        let dy = move_state.delta[1].wrapping_abs() as u32;
        let dz = move_state.delta[2].wrapping_abs() as u32;

        // --- Bresenham's Line Algorithm ---
        if dx >= dy && dx >= dz { 
            // X is dominant axis
            move_state.err[0] += dy;
            move_state.err[1] += dz;

            if move_state.err[0] >= dx {
                move_state.err[0] -= dx;
                step_high_state = step_high_state.step_y();
                let sign = move_state.delta[1].signum();
                self.current_pos[1] += sign;
                self.slew_steps[1] += sign;
            }
            if move_state.err[1] >= dx {
                move_state.err[1] -= dx;
                step_high_state = step_high_state.step_z();
                let sign = move_state.delta[2].signum();
                self.current_pos[2] += sign;
                self.slew_steps[2] += sign;
            }
            // Always step the dominant axis
            step_high_state = step_high_state.step_x();
            let sign = move_state.delta[0].signum();
            self.current_pos[0] += sign;
            self.slew_steps[0] += sign;

        } else if dy >= dx && dy >= dz { 
            // Y is dominant axis
            move_state.err[0] += dx;
            move_state.err[1] += dz;

            if move_state.err[0] >= dy {
                move_state.err[0] -= dy;
                step_high_state = step_high_state.step_x();
                let sign = move_state.delta[0].signum();
                self.current_pos[0] += sign;
                self.slew_steps[0] += sign;
            }
            if move_state.err[1] >= dy {
                move_state.err[1] -= dy;
                step_high_state = step_high_state.step_z();
                let sign = move_state.delta[2].signum();
                self.current_pos[2] += sign;
                self.slew_steps[2] += sign;
            }
            // Always step the dominant axis
            step_high_state = step_high_state.step_y();
            let sign = move_state.delta[1].signum();
            self.current_pos[1] += sign;
            self.slew_steps[1] += sign;
        } else { 
            // Z is dominant axis
            move_state.err[0] += dx;
            move_state.err[1] += dy;

            if move_state.err[0] >= dz {
                move_state.err[0] -= dz;
                step_high_state = step_high_state.step_x();
                let sign = move_state.delta[0].signum();
                self.current_pos[0] += sign;
                self.slew_steps[0] += sign;
            }
            if move_state.err[1] >= dz { // Note: was `move_state.err[1] >= dy` before, dz is correct
                move_state.err[1] -= dz;
                step_high_state = step_high_state.step_y();
                let sign = move_state.delta[1].signum();
                self.current_pos[1] += sign;
                self.slew_steps[1] += sign;
            }
            // Always step the dominant axis
            step_high_state = step_high_state.step_z();
            let sign = move_state.delta[2].signum();
            self.current_pos[2] += sign;
            self.slew_steps[2] += sign;
        }

        // --- Store the byte for logging ---
        LAST_STEP_BYTE.store(step_high_state.0, Ordering::Relaxed);
        //println!("Step High State: {:08b}", step_high_state.0);
        //println!("X Step: {}, Y Step: {}, Z Step: {}", (step_high_state.0 & X_STEP_PIN) != 0, (step_high_state.0 & Y_STEP_PIN) != 0, (step_high_state.0 & Z_STEP_PIN) != 0);
        //println!("Sent step low state: {:08b}", self.base_state.step_low().0);
        // Send the pulse to the hardware driver.
        self.driver.send_steps(&[step_high_state.0, self.base_state.step_low().0])?;
        
        Ok(())
    }

    /// Helper for continuous moves (Tracking/Guiding). Called by `update`, so it MUST be in IRAM.
    /// This function is integer-only and panic-free.
    #[inline(never)]
    #[link_section = ".iram1.text"]
    fn update_continuous_move(&mut self) -> Result<(), EspError> {
        
        let counter: &mut [i32; 3] = if self.mode == PlannerMode::Tracking { &mut self.tracking_steps } else { &mut self.guiding_steps };

        // Use the pre-calculated integer values from the state struct.
        let numerators = self.cont_numerators;
        let directions = self.cont_directions;
        
        const DENOMINATOR: i128 = 1_000_000_000_000;

        let mut step_high_state = self.base_state;
        let mut stepped = false;

        // --- The DDA Core Logic (Integer-Only) ---
        for i in 0..3 {
            self.rate_accumulators[i] += numerators[i];
            
            if self.rate_accumulators[i] >= DENOMINATOR {
                self.rate_accumulators[i] -= DENOMINATOR;
                
                step_high_state = match i {
                    0 => step_high_state.step_x(),
                    1 => step_high_state.step_y(),
                    2 => step_high_state.step_z(),
                    _ => unreachable!(),
                };
                
                self.current_pos[i] += directions[i];
                counter[i] += directions[i];
                stepped = true;
            }
        }

        if stepped {

            // --- Store the byte for logging ---
            LAST_STEP_BYTE.store(step_high_state.0, Ordering::Relaxed);
            //println!("Step High State: {:08b}", step_high_state.0);
            //println!("X Step: {}, Y Step: {}, Z Step: {}", (step_high_state.0 & X_STEP_PIN) != 0, (step_high_state.0 & Y_STEP_PIN) != 0, (step_high_state.0 & Z_STEP_PIN) != 0);
            //println!("Sent step low state: {:08b}", self.base_state.step_low().0);
            self.driver.send_steps(&[step_high_state.0, self.base_state.step_low().0])?;
        }
        
        Ok(())
    }


    // --- Public API / Command Methods (Not in IRAM) ---
    pub fn move_steps(&mut self, delta: [i32; 3]) -> Result<(), &'static str> {
        let target = [
            self.current_pos[0].saturating_add(delta[0]),
            self.current_pos[1].saturating_add(delta[1]),
            self.current_pos[2].saturating_add(delta[2]),
        ];
        self.move_to_steps(target)
    }

    /// Prepares and initiates a finite, point-to-point slew to an absolute target position.
    pub fn move_to_steps(&mut self, target_steps: [i32; 3]) -> Result<(), &'static str> {
        // Guard against starting a new move while another is in progress.
        if self.is_busy() {
            return Err("Planner is busy.");
        }

        // Calculate the total distance (delta) for each axis.
        let delta = [
            target_steps[0] - self.current_pos[0],
            target_steps[1] - self.current_pos[1],
            target_steps[2] - self.current_pos[2],
        ];

        // If there's no distance to move, we're done.
        if delta.iter().all(|&d| d == 0) {
            self.mode = PlannerMode::Idle; // Go to Idle if no move is needed.
            return Ok(());
        }

        self.base_state = MotorState::new().enable_motors(true)
                                      .set_x_dir(delta[0] >= 0)
                                      .set_y_dir(delta[1] >= 0)
                                      .set_z_dir(delta[2] >= 0);
        

        // Find the dominant axis for the Bresenham's algorithm.
        let total_steps_dominant = delta.iter().map(|d| d.abs()).max().unwrap_or(0);
        
        // Create the initial state object for the finite move.
        let move_state = MoveState {
            target: target_steps,
            delta,
            // Initialize the Bresenham's error terms.
            err: [(total_steps_dominant / 2) as u32; 2],
        };

        self.move_data = move_state;

        println!("[PLANNER]  - Current Pos: {:?}", self.current_pos);
        println!("[PLANNER]  - Target Pos:  {:?}", target_steps);
        println!("[PLANNER]  - Calculated Delta: {:?}", delta);
        //println!("[PLANNER]  - Accel/Cruise/Decel Steps: {}/{}/{}", steps_to_accel, steps_to_cruise, steps_to_decel);

        self.slew_steps = [0, 0, 0]; // Reset slew counter for the new move
        self.mode = PlannerMode::Moving;
        
        Ok(())
    }

    /// Sets the planner to a continuous tracking rate.
    /// This function performs all necessary floating-point calculations to prepare
    /// the fixed-point values for the IRAM-safe `update` function.
    pub fn track_rate_sps(&mut self, rates_sps: [f64; 3]) -> Result<(), &'static str> {
        if self.is_slewing() {
            return Err("Planner is busy with a slew.");
        }

        // If the new rates are all zero, stop motion and go to Idle.
        if rates_sps.iter().all(|&r| r == 0.0) {
            self.mode = PlannerMode::Idle;
            return Ok(());
        }
        // Start with a clean slate for the base state.
        // Get a state with ONLY the enable bit set correctly.
        let mut new_base_state = MotorState::new().enable_motors(true);

        // Now, set the directions for THIS specific move.
        new_base_state = new_base_state.set_x_dir(rates_sps[0] >= 0.0);
        new_base_state = new_base_state.set_y_dir(rates_sps[1] >= 0.0);
        // Note: we are NOT setting a Z direction. It will remain 0.
        
        // Assign the newly constructed, clean state.
        self.base_state = new_base_state;

        // --- Pre-calculation Step (Floating-Point Math is SAFE here) ---
        let pulse_rate_hz = self.update_frequency_hz as f64;
        
        // Use a larger denominator for higher precision in fixed-point math.
        const DENOMINATOR: i128 = 1_000_000_000_000;
        
        // Calculate the integer "numerator" or error term to add on each ISR tick.
        self.cont_numerators = [
            ((rates_sps[0].abs() * DENOMINATOR as f64) / pulse_rate_hz).round() as i128,
            ((rates_sps[1].abs() * DENOMINATOR as f64) / pulse_rate_hz).round() as i128,
            ((rates_sps[2].abs() * DENOMINATOR as f64) / pulse_rate_hz).round() as i128,
        ];
        
        // Pre-calculate and store the direction of movement.
        self.cont_directions = [
            rates_sps[0].signum() as i32,
            rates_sps[1].signum() as i32,
            rates_sps[2].signum() as i32,
        ];

        // If we are starting from a complete stop, reset the DDA error accumulators.
        // This prevents carrying over old fractional errors.
        if self.mode == PlannerMode::Idle {
            self.rate_accumulators = [0; 3];
        }

        // Atomically set the new state.
        self.mode = PlannerMode::Tracking;
        
        // Set the physical direction pins.
        self.base_state = self.base_state.set_x_dir(rates_sps[0] >= 0.0)
                                        .set_y_dir(rates_sps[1] >= 0.0);
        Ok(())
    }

    /// Sets the planner to a continuous guiding rate.
    /// Guiding is a special case of continuous movement.
    /// Prepares and initiates a continuous move for a guide pulse.
    pub fn guide_rate_sps(&mut self, rates_sps: [f64; 3]) -> Result<(), &'static str> {
        if self.is_slewing() {
            return Err("Planner is busy with a slew.");
        }

        // Start with a clean slate for the base state.
        // Get a state with ONLY the enable bit set correctly.
        let mut new_base_state = MotorState::new().enable_motors(true);

        // Now, set the directions for THIS specific move.
        new_base_state = new_base_state.set_x_dir(rates_sps[0] >= 0.0);
        new_base_state = new_base_state.set_y_dir(rates_sps[1] >= 0.0);
        // Note: we are NOT setting a Z direction. It will remain 0.
        
        // Assign the newly constructed, clean state.
        self.base_state = new_base_state;

        // --- Pre-calculation Step ---
        let pulse_rate_hz = self.update_frequency_hz as f64;
        const DENOMINATOR: i128 = 1_000_000_000_000;
        
        self.cont_numerators = [
            ((rates_sps[0].abs() * DENOMINATOR as f64) / pulse_rate_hz).round() as i128,
            ((rates_sps[1].abs() * DENOMINATOR as f64) / pulse_rate_hz).round() as i128,
            ((rates_sps[2].abs() * DENOMINATOR as f64) / pulse_rate_hz).round() as i128,
        ];
        
        self.cont_directions = [
            rates_sps[0].signum() as i32,
            rates_sps[1].signum() as i32,
            rates_sps[2].signum() as i32,
        ];
        
        // IMPORTANT: Guiding MODIFIES an existing track. It should NOT reset the
        // accumulators unless the mount was completely idle. This preserves the
        // fractional step error from the previous tracking tick, ensuring a
        // seamless, high-precision transition from tracking to guiding.
        if self.mode == PlannerMode::Idle {
            self.rate_accumulators = [0; 3];
        }

        // Atomically set the new state to Guiding.
        self.mode = PlannerMode::Guiding;
        
        // Set the physical direction pins.
        self.base_state = self.base_state.set_x_dir(rates_sps[0] >= 0.0)
                                        .set_y_dir(rates_sps[1] >= 0.0)
                                        .set_z_dir(rates_sps[2] >= 0.0);
        Ok(())
    }

    /// Immediately stops all motion and forces the planner into an Idle state.
    /// This is a high-priority command.
    pub fn stop(&mut self) {
        // We simply replace the current state with the Idle variant.
        self.mode = PlannerMode::Idle;
    }

    // --- State Query Methods ---

    pub fn is_slewing(&self) -> bool { self.mode == PlannerMode::Moving }
    pub fn is_tracking(&self) -> bool { self.mode == PlannerMode::Tracking }
    pub fn is_guiding(&self) -> bool { self.mode == PlannerMode::Guiding }
    pub fn is_busy(&self) -> bool { self.mode != PlannerMode::Idle }

    pub fn sync_to_position(&mut self, new_logical_position: [i32; 3]) {
        self.sync_steps[0] = self.current_pos[0] - new_logical_position[0];
        self.sync_steps[1] = self.current_pos[1] - new_logical_position[1];
        self.sync_steps[2] = self.current_pos[2] - new_logical_position[2];
    }

    pub fn current_position_steps(&self) -> [i32; 3] {
        self.current_pos
    }
    
    /// Gets the total steps taken only during tracking.
    pub fn tracking_steps(&self) -> [i32; 3] {
        self.tracking_steps
    }

    /// Gets the total steps taken only during guiding.
    pub fn guiding_steps(&self) -> [i32; 3] {
        self.guiding_steps
    }

    /// Gets the total steps taken only during slews (`move_to` or `move_by`).
    pub fn slew_steps(&self) -> [i32; 3] {
        self.slew_steps
    }

    /// Gets the total steps taken during sync operations.
    /// This is the number of steps moved when calling `sync_to_degrees`.
    pub fn sync_steps(&self) -> [i32; 3] {
        self.sync_steps
    }

    // Resets the tracking step counter to zero.
    pub fn reset_tracking_steps(&mut self) {
        self.tracking_steps = [0, 0, 0];
    }

    // Resets the guiding step counter to zero.
    pub fn reset_guiding_steps(&mut self) {
        self.guiding_steps = [0, 0, 0];
        
    }

    /// Resets the slew step counter to zero.
    pub fn reset_slew_steps(&mut self) {
        self.slew_steps = [0; 3];
    }

    /// Resets the sync step counter to zero.
    pub fn reset_sync_steps(&mut self) {
        self.sync_steps = [0; 3];
    }

    pub fn get_last_step_byte(&self) -> u8 {
        LAST_STEP_BYTE.load(Ordering::Relaxed)
    }

    pub fn get_snapshot(&self) -> (PlannerMode, [i32; 3]) {
        (self.mode, self.current_pos)
    }
    
    // ... other telemetry getters if needed ...
}