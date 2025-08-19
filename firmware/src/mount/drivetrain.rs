// src/mount/drivetrain.rs

// This is the new, unified struct.
#[derive(Debug, Clone, Copy)]
pub struct AxisDrivetrain {
    // --- Physical Inputs ---
    pub steps_per_rev_motor: f64,
    pub microstepping: u32,
    pub total_gear_ratio: f64,

    // --- Calculated Properties ---
    pub steps_per_degree: f64,
    /// The celestial coordinate (in degrees) that corresponds to 0 motor steps.
    pub home_offset_degrees: f64,
}

impl AxisDrivetrain {
    pub fn new(steps_per_rev_motor: u32, microstepping: u32, total_gear_ratio: f64, home_offset_degrees: f64) -> Self {
        let steps_per_rev_final_axis = (steps_per_rev_motor * microstepping) as f64 * total_gear_ratio;
        let steps_per_degree = steps_per_rev_final_axis / 360.0;

        //println!("AxisDrivetrain: steps_per_rev_motor: {}, microstepping: {}, total_gear_ratio: {}, steps_per_degree: {}, home_offset_degrees: {}",
        //    steps_per_rev_motor, microstepping, total_gear_ratio, steps_per_degree, home_offset_degrees);

        Self {
            steps_per_rev_motor: steps_per_rev_motor as f64,
            microstepping: microstepping as u32,
            total_gear_ratio,
            steps_per_degree,
            home_offset_degrees,
        }
    }
    
    // --- Conversion Methods ---
    pub fn degrees_to_steps(&self, degrees: f64) -> i32 {
        // Adjust for home offset
        let adjusted_degrees = degrees - self.home_offset_degrees;
        // Convert degrees to steps
        (adjusted_degrees * self.steps_per_degree).round() as i32
    }

    pub fn steps_to_degrees(&self, steps: i32) -> f64 {
        steps as f64 / self.steps_per_degree + self.home_offset_degrees
    }
    
    // A specific method for RA tracking rate
    pub fn get_sidereal_tracking_rate_sps(&self) -> f64 {
        //const SIDEREAL_HOURS_PER_DAY: f64 = 23.9344699;
        const SIDEREAL_SECONDS_IN_MEAN_DAY: f64 = 86164.0905;
        let steps_per_rev_final_axis = self.steps_per_rev_motor * self.microstepping as f64 * self.total_gear_ratio;
        let steps_per_sidereal_day = steps_per_rev_final_axis;
        let result = steps_per_sidereal_day / 86400.0;
        //let result = steps_per_sidereal_day / (SIDEREAL_SECONDS_IN_MEAN_DAY);
        result
    }

    // NEW: A method for converting a relative angle to relative steps.
    // This is different from the absolute conversion. It doesn't use the home_offset.
    pub fn degrees_to_steps_relative(&self, degrees: f64) -> i32 {
        (degrees * self.steps_per_degree).round() as i32
    }

    // Print summary information
    pub fn print_summary(&self) {
        println!("===================================");
        println!("  Steps per Revolution of the Motor: {}", self.steps_per_rev_motor);
        println!("  Microstepping: {}", self.microstepping);
        println!("  Total Gear Ratio: {}", self.total_gear_ratio);
        println!("  Steps per Degree: {}", self.steps_per_degree);
        println!("  Home Offset Degrees: {}", self.home_offset_degrees);
    }
}