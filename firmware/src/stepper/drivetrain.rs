// src/stepper/drivetrain.rs

use super::i2s0::I2sStepperDriver;
use super::planner::MotionPlanner;
use esp_idf_svc::sys::EspError;

/// Configuration for a single axis of the drivetrain.
#[derive(Clone, Copy)]
pub struct AxisConfig {
    /// Steps per full revolution of the motor shaft (e.g., 200 for a 1.8° motor).
    pub steps_per_revolution: u32,
    /// Microstepping setting on the driver (e.g., 16, 32).
    pub microstepping: u32,
    /// The gear ratio between the motor and the final output.
    pub gear_ratio: f64,
}

impl AxisConfig {
    /// Calculates the number of steps required to move one degree.
    fn steps_per_degree(&self) -> f64 {
        (self.steps_per_revolution as f64 * self.microstepping as f64 * self.gear_ratio) / 360.0
    }
}



/// The main Drivetrain controller.
/// This is the primary interface for your application.
/// It operates in real-world units (degrees).
pub struct TelescopeDrive<'d> {
    planner: MotionPlanner<'d>,
    steps_per_degree: [f64; 3], // [X, Y, Z]
}

impl<'d> TelescopeDrive<'d> {
    pub fn new(
        driver: I2sStepperDriver<'d>,
        sample_rate: u32,
        axis_configs: [AxisConfig; 3],
    ) -> Self {
        Self {
            planner: MotionPlanner::new(driver, sample_rate),
            steps_per_degree: [
                axis_configs[0].steps_per_degree(),
                axis_configs[1].steps_per_degree(),
                axis_configs[2].steps_per_degree(),
            ],
        }
    }
    
    /// Sets the motion profile parameters for the planner.
    /// `max_speed` is in degrees/sec.
    /// `acceleration` is in degrees/sec^2.
    pub fn set_motion_profile(&mut self, axis_idx: usize, max_speed: f64, acceleration: f64) {
        // Convert public units (degrees) to internal units (steps)
        let max_speed_sps = max_speed * self.steps_per_degree[axis_idx];
        let acceleration_sps2 = acceleration * self.steps_per_degree[axis_idx];
        self.planner.set_profile(axis_idx, max_speed_sps as f32, acceleration_sps2 as f32);
    }

    /// Moves to an absolute position specified in degrees.
    pub fn move_to_position(&mut self, target_degrees: [f64; 3]) -> Result<(), &'static str> {
        let target_steps = [
            (target_degrees[0] * self.steps_per_degree[0]) as i32,
            (target_degrees[1] * self.steps_per_degree[1]) as i32,
            (target_degrees[2] * self.steps_per_degree[2]) as i32,
        ];
         // We calculate the delta from the *logical* position, but the `move_to` in the
        // planner needs a *physical* target.
        let logical_pos = self.planner.get_logical_position();
        let delta_steps = [
            target_steps[0] - logical_pos[0],
            target_steps[1] - logical_pos[1],
            target_steps[2] - logical_pos[2],
        ];

        // The physical target is the current physical position + the calculated delta.
        let physical_target = [
            self.planner.current_pos[0] + delta_steps[0],
            self.planner.current_pos[1] + delta_steps[1],
            self.planner.current_pos[2] + delta_steps[2],
        ];
        self.planner.move_to(physical_target)
    }

    /// Moves by a relative distance in steps.
    pub fn move_by_steps(&mut self, delta_steps: [i32; 3]) -> Result<(), &'static str> {
        // Convert the delta steps to the physical target position
        let physical_target = [
            self.planner.current_pos[0] + delta_steps[0],
            self.planner.current_pos[1] + delta_steps[1],
            self.planner.current_pos[2] + delta_steps[2],
        ];
        self.planner.move_to(physical_target)
    }

    /// Moves by a relative angle in degrees.
    pub fn move_by_angle(&mut self, delta_degrees: [f64; 3]) -> Result<(), &'static str> {
        let delta_steps = [
            (delta_degrees[0] * self.steps_per_degree[0]) as i32,
            (delta_degrees[1] * self.steps_per_degree[1]) as i32,
            (delta_degrees[2] * self.steps_per_degree[2]) as i32,
        ];
        self.planner.move_steps(delta_steps)
    }

    /// Starts continuous tracking. Call `reset_tracking_counter` first if desired.
    pub fn track_rate(&mut self, rates_dps: [f64; 3]) -> Result<(), &'static str> {
        let rates_sps = self.convert_dps_to_sps(rates_dps);
        self.planner.move_steady(rates_sps) // move_steady is now our tracking method
    }
    
    /// Issues a guiding command. Guiding steps are counted separately.
    pub fn guide_rate(&mut self, rates_dps: [f64; 3]) -> Result<(), &'static str> {
        let rates_sps = self.convert_dps_to_sps(rates_dps);
        self.planner.guide_rate(rates_sps)
    }
    
    // Helper function to avoid repetition
    fn convert_dps_to_sps(&self, rates_dps: [f64; 3]) -> [f32; 3] {
        [
            (rates_dps[0] * self.steps_per_degree[0]) as f32,
            (rates_dps[1] * self.steps_per_degree[1]) as f32,
            (rates_dps[2] * self.steps_per_degree[2]) as f32,
        ]
    }

    /// Gets the total steps taken only during tracking.
    pub fn tracking_steps(&self) -> [i32; 3] {
        self.planner.tracking_steps
    }

    /// Gets the total steps taken only during guiding.
    pub fn guiding_steps(&self) -> [i32; 3] {
        self.planner.guiding_steps
    }

    /// Gets the total steps taken only during slews (`move_to` or `move_by`).
    pub fn slew_steps(&self) -> [i32; 3] {
        self.planner.slew_steps
    }

    /// Gets the total steps taken during sync operations.
    /// This is the number of steps moved when calling `sync_to_degrees`.
    pub fn sync_steps(&self) -> [i32; 3] {
        self.planner.sync_steps
    }

    /// Resets the tracking counter.
    pub fn reset_tracking_counter(&mut self) {
        self.planner.reset_tracking_steps();
    }
    
    /// Resets the guiding counter.
    pub fn reset_guiding_counter(&mut self) {
        self.planner.reset_guiding_steps();
    }

    /// Resets the slew counter.
    pub fn reset_slew_counter(&mut self) {
        self.planner.reset_slew_counter();
    }

    /// Syncs the mount's internal coordinates. Call this when the telescope is
    /// physically pointing at a known celestial object whose coordinates (in degrees
    /// from the home position) are `new_logical_degrees`.
    pub fn sync_to_degrees(&mut self, new_logical_degrees: [f64; 3]) {
        // Convert the target degrees into target steps.
        let new_logical_steps = [
            (new_logical_degrees[0] * self.steps_per_degree[0]) as i32,
            (new_logical_degrees[1] * self.steps_per_degree[1]) as i32,
            (new_logical_degrees[2] * self.steps_per_degree[2]) as i32,
        ];
        
        self.planner.sync_to_position(new_logical_steps);
    }
    
    /// Gets the current position of each axis in degrees.
    pub fn current_position_degrees(&self) -> [f64; 3] {
        [
            self.planner.current_pos[0] as f64 / self.steps_per_degree[0],
            self.planner.current_pos[1] as f64 / self.steps_per_degree[1],
            self.planner.current_pos[2] as f64 / self.steps_per_degree[2],
        ]
    }
    /// Gets the current position of each axis in steps.
    pub fn current_position_steps(&self) -> [i32; 3] {
        [
            self.planner.current_pos[0],
            self.planner.current_pos[1],
            self.planner.current_pos[2],
        ]
    }

    // --- Pass-through methods ---
    pub fn update(&mut self) -> Result<(), EspError> { self.planner.update() }
    pub fn stop(&mut self) { self.planner.stop() }
    pub fn is_busy(&self) -> bool { self.planner.is_busy() }
}