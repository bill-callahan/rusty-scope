// src/mount/rightascension.rs
//! This module defines the `RightAscension` struct and its associated methods for handling Right Ascension (RA) coordinates in sidereal time.
//! It includes normalization, conversion to/from hours, minutes, seconds, and motor steps, as well as arithmetic operations.
//! The `DrivetrainRA` struct represents the drivetrain for the RA axis, including gear ratios and steps per revolution.

use crate::mount::{app_state::PierSide, AxisDrivetrain};


// --- CENTRALIZED SIDEREAL TIME CONSTANTS ---
// Using IAU 2000 / IERS 2010 conventions for sidereal day length.
// The precise length of a mean sidereal day is 23h 56m 4.090530833s
pub const SIDEREAL_HOURS_PER_DAY: f64 = 23.9344699; // Mean Sidereal Day in hours
pub const SIDEREAL_SECONDS_PER_DAY: f64 = SIDEREAL_HOURS_PER_DAY * 3600.0;
pub const SECONDS_PER_HOUR: f64 = 3600.0;
pub const SECONDS_PER_MINUTE: f64 = 60.0;

// Helper functions (assuming these exist or you'd add them)
// From your previous `compute_sidereal_time` function:
pub fn rad_to_hours(radians: f64) -> f64 {
    radians * (12.0 / std::f64::consts::PI)
}

pub fn deg_to_hours(degrees: f64) -> f64 {
    degrees / 15.0 // 15 degrees = 1 hour of RA
}

// A more robust modulo for floats that always returns a non-negative result
pub fn modulo(value: f64, modulus: f64) -> f64 {
    value.rem_euclid(modulus)
}


/// Represents a Right Ascension coordinate in the range [0, SIDEREAL_SECONDS_PER_DAY).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)] // Add PartialEq, PartialOrd for comparisons
pub struct RightAscension {
    pub seconds: f64,
}

impl RightAscension {
    /// Creates a new `RightAscension` value from a floating-point number representing the
    /// right ascension in sidereal seconds.
    pub fn new(total_seconds: f64) -> Self {
        RightAscension { seconds: total_seconds }.normalize_to_seconds()
    }

    /// Creates a new `RightAscension` value from a floating-point number representing hours.
    pub fn new_from_hours(hours: f64) -> Self {
        let total_seconds = hours * SECONDS_PER_HOUR;
        RightAscension::new(total_seconds)
    }

    /// Creates a new `RightAscension` value from a tuple of hours, minutes, and seconds.
    pub fn new_from_hms(hours: i32, minutes: i32, seconds: f64) -> Self {
        let total_seconds = (hours as f64 * SECONDS_PER_HOUR) + (minutes as f64 * SECONDS_PER_MINUTE) + seconds;
        RightAscension::new(total_seconds)
    }

    /// Creates a new `RightAscension` value from a string representing a right ascension in a string like "12:34:56.78"
    /// Returns a `Result` for robust parsing.
    pub fn new_from_hms_string(hms_string: &str) -> Result<Self, String> {
        let parts: Vec<&str> = hms_string.split(':').collect();
        if parts.len() != 3 {
            return Err("Invalid format. Expected HH:MM:SS".to_string());
        }

        let hours = parts[0].parse::<i32>().map_err(|_| "Invalid hours".to_string())?;
        let minutes = parts[1].parse::<i32>().map_err(|_| "Invalid minutes".to_string())?;
        let seconds = parts[2].parse::<f64>().map_err(|_| "Invalid seconds".to_string())?;

        Ok(RightAscension::new_from_hms(hours, minutes, seconds))
    }



    /// Creates a new `RightAscension` value from motor steps.
    /// Requires an instance of `DrivetrainRA` to calculate seconds per step.
    /// Requires an instance of `AxisDrivetrain` to calculate degrees per step.
    pub fn new_from_motor_steps(
        steps: i32, 
        lst_hours: f64,
        drivetrain: &AxisDrivetrain
        ) -> Self {
        let mount_degrees = drivetrain.steps_to_degrees(steps);
        let ha_hours = mount_degrees / 15.0;
        // RA = LST - HA
        Self::new_from_hours(lst_hours - ha_hours)
    }

    /// Normalizes the right ascension value to be between 0 and SIDEREAL_SECONDS_PER_DAY
    fn normalize_to_seconds(&self) -> Self {
        // Use rem_euclid for robust non-negative modulo
        RightAscension { seconds: self.seconds.rem_euclid(SIDEREAL_SECONDS_PER_DAY) }
    }

    /// Returns the right ascension value in sidereal seconds.
    pub fn total_seconds(&self) -> f64 {
        self.seconds
    }

    /// Returns the right ascension value in sidereal hours.
    pub fn total_hours(&self) -> f64 {
        self.seconds / SECONDS_PER_HOUR
    }

    /// Returns the right ascension value as a tuple of hours, minutes, and seconds.
    pub fn as_hms(&self) -> (i32, i32, f64) {
        let total_hours = self.total_hours();
        let hours = total_hours.trunc() as i32;
        let minutes = ((total_hours.fract() * SECONDS_PER_HOUR) / SECONDS_PER_MINUTE).trunc() as i32;
        let seconds = (total_hours.fract() * SECONDS_PER_HOUR).rem_euclid(SECONDS_PER_MINUTE);
        (hours, minutes, seconds)
    }

    /// Returns the right ascension value as a string in the format "HH:MM:SS.SS"
    pub fn as_hms_string(&self) -> String {
        let (hours, minutes, seconds) = self.as_hms();
        let ttt = format!("{:02}:{:02}:{:05.2}", hours, minutes, seconds);
        ttt
    }

    /// Returns the right ascension value as motor steps.
    /// Requires an instance of `DrivetrainRA` to calculate steps per second.
    /// Requires an instance of `AxisDrivetrain` to calculate steps per degree.
    /// Returns the required absolute motor step position for this Right Ascension
    /// at a specific Local Sidereal Time.
    pub fn as_motor_steps(&self, lst_hours: f64, drivetrain: &AxisDrivetrain) -> i32 {
        // 1. Convert the celestial coordinate (RA) to a physical orientation (HA).
        let ha_hours = self.to_hour_angle_hours(lst_hours);

        // 2. Convert the physical orientation (HA in hours) to a physical angle (degrees).
        let mount_degrees = ha_hours * 15.0;

        // 3. Convert the physical angle to an absolute motor step position.
        drivetrain.degrees_to_steps(mount_degrees)
    }

    /// Calculates the required absolute motor step position for this RA at a specific LST,
    /// considering the desired side of the pier.
    pub fn as_motor_steps_for_pier_side(
        &self,
        lst_hours: f64,
        drivetrain: &AxisDrivetrain,
        side: PierSide,
    ) -> i32 {
        let mut ha_hours = self.to_hour_angle_hours(lst_hours);

        // --- The Key Logic ---
        // A GEM cannot physically point to HA > 0 on the East side, or HA < 0 on the West.
        // If the requested side doesn't match the HA, we must flip the mount
        // by adding/subtracting 12 hours from the HA to get the equivalent
        // orientation on the other side of the pier.
        match side {
            PierSide::East if ha_hours > 0.0 => { // Target is West, but we want to be on East side
                ha_hours -= 12.0;
            }
            PierSide::West if ha_hours < 0.0 => { // Target is East, but we want to be on West side
                ha_hours += 12.0;
            }
            _ => { /* No adjustment needed */ }
        }

        let mount_degrees = ha_hours * 15.0;
        drivetrain.degrees_to_steps(mount_degrees)
    }

    /// Calculates the Hour Angle (HA) for this Right Ascension given the current Local Sidereal Time (LST).
    /// HA = LST - RA.
    /// Returns HA in hours, normalized to be between -12 and +12 hours,
    /// where positive HA is West of the meridian and negative HA is East.
    pub fn to_hour_angle_hours(&self, lst_hours: f64) -> f64 {
        let ra_hours = self.total_hours();
        let mut ha = lst_hours - ra_hours;

        // Normalize HA to be between -SIDEREAL_HOURS_PER_DAY/2 and +SIDEREAL_HOURS_PER_DAY/2
        if ha > SIDEREAL_HOURS_PER_DAY / 2.0 {
            ha -= SIDEREAL_HOURS_PER_DAY;
        } else if ha < -SIDEREAL_HOURS_PER_DAY / 2.0 {
            ha += SIDEREAL_HOURS_PER_DAY;
        }
        ha
    }
}

// Add the helper function to your file
fn get_dummy_parse_float_error() -> std::num::ParseFloatError {
    "".parse::<f64>().unwrap_err()
}

// Implement arithmetic traits for RightAscension
// These operations will always return a normalized RA position.
impl std::ops::Add for RightAscension {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        RightAscension { seconds: self.seconds + rhs.seconds }.normalize_to_seconds()
    }
}

impl std::ops::Sub for RightAscension {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        // This calculates a "difference" that is still a normalized RA position,
        // effectively moving `rhs` relative to `self` on the RA circle.
        // For signed angular difference (like Hour Angle), use `to_hour_angle_hours`.
        RightAscension { seconds: self.seconds - rhs.seconds }.normalize_to_seconds()
    }
}

impl std::ops::AddAssign for RightAscension {
    fn add_assign(&mut self, rhs: Self) {
        self.seconds += rhs.seconds;
        *self = self.normalize_to_seconds();
    }
}

impl std::ops::SubAssign for RightAscension {
    fn sub_assign(&mut self, rhs: Self) {
        self.seconds -= rhs.seconds;
        *self = self.normalize_to_seconds();
    }
}


// Assuming SIDEREAL_HOURS_PER_DAY, SECONDS_PER_MINUTE are available from a common constants file/module.
// Example:
// pub const SIDEREAL_HOURS_PER_DAY: f64 = 23.9344699;
// pub const SECONDS_PER_MINUTE: f64 = 60.0;
// pub const SECONDS_PER_HOUR: f64 = 3600.0;

/// The drivetrain for the RA axis.
#[derive(Debug, Clone, Copy)]
pub struct DrivetrainRA {
    pub nema17_steps_per_rev: f64,    // Steps per revolution of the Nema17 motor
    pub microstepping_levels: f64,    // Microstepping setting (e.g., 1, 2, 4, ..., 128)
    pub motor_gearing: f64,  // Internal gearbox ratio of the motor itself (e.g., 100 for a 1:100 geared stepper)

    // New fields for up to 4 stages of external gearing
    pub gear_ratio_stage_1: f64, // e.g., worm gear ratio
    pub gear_ratio_stage_2: f64, // e.g., reduction stage 2
    pub gear_ratio_stage_3: f64, // e.g., reduction stage 3
    pub gear_ratio_stage_4: f64, // e.g., reduction stage 4

    pub total_gear_ratio: f64,        // Combined ratio from motor to final RA axis
    pub total_steps_per_motor_rev: f64, // Effective steps per motor revolution after microstepping

    pub steps_per_rev_final_axis: f64, // Total steps per revolution of the final RA axis (motor to final axis)
    pub steps_per_degree: f64,   // Steps per degree of RA axis rotation - assuming 360 degrees total
    pub steps_per_hour: f64,     // Steps per hour of RA axis rotation - assuming Sidereal Day
    pub steps_per_minute: f64,   // Steps per minute of RA axis rotation - assuming Sidereal Day
    pub steps_per_second: f64,   // Steps per second of RA axis rotation - assuming Sidereal Day
    pub frequency_per_step: f64,
}

impl DrivetrainRA {
    pub fn new() -> Self {
        let result = Self::default();

        

        println!("========== RA Drivetrain data =======================");
        println!("Nema17 Steps/Rev: {}", result.nema17_steps_per_rev);
        println!("Microsteps: {} | Motor Gearing: {}:1", result.microstepping_levels, result.motor_gearing);
        println!("Total Steps per Motor Revolution (Microstepped): {}", result.total_steps_per_motor_rev);
        println!("Step pulse = {} us    Steps delay = {} us", 
            1000000.0 / (result.frequency_per_step * result.total_steps_per_motor_rev),
            1000000.0 / result.frequency_per_step);
        println!("------------------------------------------------------");
        println!("Gears");
        println!("Gear Ratios:  MG[{}:1]   G1[{}:1]  G2[{}:1]  G3[{}:1]  G4[{}:1]", result.motor_gearing,
            result.gear_ratio_stage_1, result.gear_ratio_stage_2, result.gear_ratio_stage_3, result.gear_ratio_stage_4);
        println!("Total Gear Ratio (Motor to Final Axis):          {}:1", result.total_gear_ratio);
        println!("------------------------------------------------------");
        println!("Final Axis Steps: {:.2} steps/rev ({:.4} steps/degree)", result.steps_per_rev_final_axis, result.steps_per_degree);
        println!("------------------------------------------------------");
        println!("Sidereal Day: {:.4} hours ({:.2} minutes / {:.2} seconds)",
            SIDEREAL_HOURS_PER_DAY, SIDEREAL_HOURS_PER_DAY * SECONDS_PER_MINUTE, SIDEREAL_HOURS_PER_DAY * SECONDS_PER_HOUR);
        println!("Steps per Sidereal Hour:             {:.2}", result.steps_per_hour);
        println!("Steps per Sidereal Minute (of time): {:.2}", result.steps_per_minute);
        println!("Steps per Sidereal Second (of time): {:.4}", result.steps_per_second);
        println!("Sidereal Tracking Rate: 1 step every {:.4} seconds ({:.2} ms)", result.frequency_per_step, result.frequency_per_step * 1000.0);
        println!("======================================================");
        //println!("");
        result
    }

    pub fn with_gearing(mut self, stage1: f64, stage2: f64) -> Self {
        self.gear_ratio_stage_1 = stage1;
        self.gear_ratio_stage_2 = stage2;
        // Recalculate everything based on the new values
        self.recalculate()
    }

    // A private helper to avoid duplicating the calculation logic
    fn recalculate(mut self) -> Self {
        self.total_steps_per_motor_rev = self.nema17_steps_per_rev * self.microstepping_levels;
        self.total_gear_ratio = self.motor_gearing * self.gear_ratio_stage_1 * self.gear_ratio_stage_2 * self.gear_ratio_stage_3 * self.gear_ratio_stage_4;
        self.steps_per_rev_final_axis = self.total_steps_per_motor_rev * self.total_gear_ratio;
        self.steps_per_degree = self.steps_per_rev_final_axis / 360.0;
        self.steps_per_hour = self.steps_per_rev_final_axis / SIDEREAL_HOURS_PER_DAY;
        self.steps_per_minute = self.steps_per_hour / 60.0;
        self.steps_per_second = self.steps_per_minute / 60.0;
        self.frequency_per_step = 1.0 / self.steps_per_second;
        self
    }    

    /// Creates an empty `DrivetrainRA` with default values.
    fn empty() -> Self {
        Self {
            nema17_steps_per_rev: 0.0,
            microstepping_levels: 0.0,
            motor_gearing: 0.0,
            gear_ratio_stage_1: 0.0,
            gear_ratio_stage_2: 0.0,
            gear_ratio_stage_3: 0.0,
            gear_ratio_stage_4: 0.0,
            total_steps_per_motor_rev: 0.0,
            total_gear_ratio: 0.0,
            steps_per_rev_final_axis: 0.0,
            steps_per_degree: 0.0,
            steps_per_hour: 0.0,
            steps_per_minute: 0.0,
            steps_per_second: 0.0,
            frequency_per_step: 0.0,
        }
    }
}

impl Default for DrivetrainRA {
    fn default() -> Self {
        let base = Self {
            nema17_steps_per_rev: 200.0,
            microstepping_levels: 64.0,
            motor_gearing: 100.0,
            gear_ratio_stage_1: 1.0,
            gear_ratio_stage_2: 1.0,
            gear_ratio_stage_3: 1.0,
            gear_ratio_stage_4: 1.0,
            // the rest will be calculated in `recalculate`
            ..DrivetrainRA::empty()
        };
        base.recalculate()
    }
        
    
}

/*// In your main control loop:
let current_lst_hours = compute_siderealtime(your_telescope_longitude);
let target_ra = RightAscension::new_from_hms(10, 0, 0.0); // Example target RA
let hour_angle_of_target = target_ra.to_hour_angle_hours(current_lst_hours);

if hour_angle_of_target > 0.0 {
    println!("Target is {:.2} hours West of the meridian.", hour_angle_of_target);
    // Move RA motor West
} else if hour_angle_of_target < 0.0 {
    println!("Target is {:.2} hours East of the meridian.", hour_angle_of_target.abs());
    // Move RA motor East
} else {
    println!("Target is on the meridian.");
} */

