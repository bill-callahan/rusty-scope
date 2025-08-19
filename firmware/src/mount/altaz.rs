// src/mount/altaz.rs
use crate::mount::AxisDrivetrain;



// A custom error type for parsing makes the code much cleaner.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    InvalidFormat,
    InvalidComponent(String),
}


/// Represents an Altitude value between -90 and 90 degrees.
///
/// 0 degrees is the horizon. +90 degrees is the zenith. -90 degrees is the nadir.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Altitude {
    /// The altitude value in degrees.
    /// This value is normalized to be between -90 and 90 degrees.
    pub degrees: f64,
}

impl Altitude {
    /// Creates a new `Altitude` value from a floating-point number representing the
    /// altitude in degrees.
    /// This constructor normalizes the value to be between -90 and 90 degrees.
    pub fn new(degrees: f64) -> Self {
        Self { degrees: degrees.clamp(-90.0, 90.0) }
    }

    /// Creates a new `Altitude` value from a tuple of degrees, minutes, and seconds.
    pub fn new_from_dms(degrees: i32, minutes: i32, seconds: f64) -> Self {
        let sign = if degrees < 0  { -1.0 } else { 1.0 };
        // Minutes and seconds should always be positive when converting from DMS
        let total_degrees = sign * (degrees.abs() as f64 + (minutes.abs() as f64 / 60.0) + (seconds.abs() / 3600.0));
        Self::new(total_degrees * sign)
    }

    /// Creates a new `Altitude` value from a string representing an altitude in a string like "+12:34:56.78"
    /// Returns a `Result` for robust parsing.
    pub fn new_from_dms_string(dms_string: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = dms_string.split(':').collect();
        if parts.len() != 3 {
            return Err(ParseError::InvalidFormat);
        }

        let degrees = parts[0].parse::<i32>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;
        let minutes = parts[1].parse::<i32>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;
        let seconds = parts[2].parse::<f64>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;

        Ok(Altitude::new_from_dms(degrees, minutes, seconds))
    }

    

    /// Creates a new `Altitude` value from motor steps.
    /// Requires an instance of `AxisDrivetrain` to calculate degrees per step.
    pub fn new_from_motor_steps(steps: i32, drivetrain: &AxisDrivetrain) -> Self {
        Self::new(drivetrain.steps_to_degrees(steps))
    }

    /// Normalizes the altitude value by clamping it to be between -90 and 90 degrees.
    /// Values outside this range will be clipped to the nearest limit.
    pub fn normalize(&self) -> Self {
        Altitude { degrees: self.degrees.clamp(-90.0, 90.0) }
    }

    /// Returns the altitude value in degrees.
    pub fn as_degrees(&self) -> f64 {
        self.degrees
    }

    /// Returns the altitude value as a tuple of degrees, minutes, and seconds.
    /// The degrees component carries the sign, minutes and seconds are always positive.
    pub fn as_dms(&self) -> (i32, i32, f64) {
        let value = self.degrees;
        let abs_value = value.abs();

        let degrees = value.trunc() as i32; // This keeps the sign for the degrees component
        let minutes = (abs_value.fract() * 60.0).trunc() as i32;
        let seconds = (abs_value.fract() * 60.0).fract() * 60.0;
        
        (degrees, minutes, seconds)
    }

    /// Returns the altitude value as a string in the format "DD° MM' SS.SS\""
    /// A leading '+' will be added for positive degrees for clarity.
    pub fn as_dms_string(&self) -> String {
        let (degrees, minutes, seconds) = self.as_dms();
        format!("{:+03}° {:02}' {:05.2}\"", degrees, minutes, seconds)
    }

    
    /// Returns the altitude value as motor steps.
    /// Requires an instance of `AxisDrivetrain` to calculate steps per degree.
    pub fn as_motor_steps(&self, drivetrain: &AxisDrivetrain) -> i32 {
        drivetrain.degrees_to_steps(self.degrees)
    }
}

// Implement arithmetic traits for Altitude
// These operations do not implicitly normalize, allowing for calculations
// where the result might temporarily be outside the [-90, 90] range.
// Call `normalize()` explicitly after arithmetic if you need a valid coordinate.
impl std::ops::Add for Altitude {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Altitude { degrees: self.degrees + rhs.degrees }
    }
}
impl std::ops::Sub for Altitude {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
            Altitude { degrees: self.degrees - rhs.degrees }
        }
}
impl std::ops::AddAssign for Altitude {
    fn add_assign(&mut self, rhs: Self) {
        self.degrees += rhs.degrees;
    }
}
impl std::ops::SubAssign for Altitude {
    fn sub_assign(&mut self, rhs: Self) {
        self.degrees -= rhs.degrees;
    }
}

/// Represents an Azimuth value between 0 and 360 degrees.
///
/// 0 degrees is North, 90 degrees is East, 180 degrees is South, 270 degrees is West.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Azimuth{
    /// The azimuth value in degrees.
    /// This value is normalized to be between 0 (inclusive) and 360 (exclusive) degrees.
    pub degrees: f64,
}

impl Azimuth {
    /// Creates a new `Azimuth` value from a floating-point number representing the
    /// azimuth in degrees.
    /// This constructor normalizes the value to be between 0 (inclusive) and 360 (exclusive) degrees.
    pub fn new(value: f64) -> Self {
        Self { degrees: value.rem_euclid(360.0) }
    }

    /// Creates a new `Azimuth` value from a tuple of degrees, minutes, and seconds.
    /// The sign of degrees in the input is handled by normalization.
    pub fn new_from_dms(degrees: i32, minutes: i32, seconds: f64) -> Self {
        // Minutes and seconds should always be positive when converting from DMS
        let value = degrees as f64 + (minutes.abs() as f64 / 60.0) + (seconds.abs() / 3600.0);
        Azimuth::new(value) // Call the `new` constructor, which will normalize
    }

    /// Creates a new `Azimuth` value from a string representing an azimuth in a string like "123:45:67.89"
    /// Returns a `Result` for robust parsing.
    pub fn new_from_dms_string(dms_string: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = dms_string.split(':').collect();
        if parts.len() != 3 {
            return Err(ParseError::InvalidFormat);
        }

        let degrees = parts[0].parse::<i32>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;
        let minutes = parts[1].parse::<i32>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;
        let seconds = parts[2].parse::<f64>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;

        Ok(Azimuth::new_from_dms(degrees, minutes, seconds))
    }

    /// Creates a new `RightAscension` value from motor steps.
    /// Requires an instance of `DrivetrainRA` to calculate seconds per step.
    pub fn new_from_motor_steps(steps: i32, drivetrain_ra: &AxisDrivetrain) -> Self {
        Self::new(drivetrain_ra.steps_to_degrees(steps))
    }

    /// Normalizes the azimuth value to be between 0 (inclusive) and 360 (exclusive) degrees.
    /// Uses `f64::rem_euclid` for robust wrapping.
    pub fn normalize(&self) -> Self {
        Azimuth { degrees: self.degrees.rem_euclid(360.0) }
    }

    /// Returns the azimuth value in degrees.
    pub fn as_degrees(&self) -> f64 {
        self.degrees // Already normalized to [0, 360)
    }

    /// Returns the azimuth value as a tuple of degrees, minutes, and seconds.
    /// All components are non-negative, as Azimuth is typically 0-360.
    pub fn as_dms(&self) -> (i32, i32, f64) {
        let value = self.degrees; // Already normalized to [0, 360)
        
        let degrees = value.trunc() as i32;
        let minutes = (value.fract() * 60.0).trunc() as i32;
        let seconds = (value.fract() * 60.0).fract() * 60.0;
        
        (degrees, minutes, seconds)
    }

    /// Returns the azimuth value as a string in the format "DDD° MM' SS.SS\""
    pub fn as_dms_string(&self) -> String {
        let (degrees, minutes, seconds) = self.as_dms();
        format!("{:03}° {:02}' {:05.2}\"", degrees, minutes, seconds)
    }

    /// Returns the cardinal or intercardinal direction for the azimuth.
    pub fn as_direction_string(&self) -> &'static str {
        if self.degrees >= 337.5 || self.degrees < 22.5 {
            "N"
        } else if self.degrees >= 22.5 && self.degrees < 67.5 {
            "NE"
        } else if self.degrees >= 67.5 && self.degrees < 112.5 {
            "E"
        } else if self.degrees >= 112.5 && self.degrees < 157.5 {
            "SE"
        } else if self.degrees >= 157.5 && self.degrees < 202.5 {
            "S"
        } else if self.degrees >= 202.5 && self.degrees < 247.5 {
            "SW"
        } else if self.degrees >= 247.5 && self.degrees < 292.5 {
            "W"
        } else { // self.0 >= 292.5 && self.0 < 337.5
            "NW"
        }
    }

    

    /// Returns the azimuth value as motor steps.
    /// Requires an instance of `AxisDrivetrain` to calculate steps per degree.
    pub fn as_motor_steps(&self, drivetrain: &AxisDrivetrain) -> i32 {
        drivetrain.degrees_to_steps(self.degrees)
    }
}

// Implement arithmetic traits for Azimuth
// These operations should automatically normalize, similar to RightAscension,
// because Azimuth is a cyclic quantity.
impl std::ops::Add for Azimuth {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Azimuth { degrees: self.degrees + rhs.degrees }.normalize()
    }
}
impl std::ops::Sub for Azimuth {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Azimuth { degrees: self.degrees - rhs.degrees }.normalize()
    }
}
impl std::ops::AddAssign for Azimuth {
    fn add_assign(&mut self, rhs: Self) {
        self.degrees += rhs.degrees;
        *self = self.normalize();
    }
}
impl std::ops::SubAssign for Azimuth {
    fn sub_assign(&mut self, rhs: Self) {
        self.degrees -= rhs.degrees;
        *self = self.normalize();
    }
}

