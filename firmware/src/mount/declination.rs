// src/mount/declination.rs

use crate::mount::drivetrain::AxisDrivetrain;

// A custom error type for parsing makes the code much cleaner.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseDecError {
    InvalidFormat,
    InvalidDegrees,
    InvalidMinutes,
    InvalidSeconds,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)] // Add PartialEq, PartialOrd
pub struct Declination {
    pub degrees: f64,
}


impl Declination {
    /// Creates a new `Declination` value from a floating-point number representing the
    /// declination in degrees.
    ///
    /// This constructor normalizes the value to be between -90 and 90 degrees using clamping.
    pub fn new(value: f64) -> Self {
        // Automatically normalize incoming values to be within the valid range [-90, 90]
        // This is important because Declination is a bounded coordinate.
        // Declination { degrees: value }.normalize_clamp() // Ensures any new Declination is valid
        Self { degrees: value.clamp(-90.0, 90.0) } // Clamping to ensure valid declination range
    }

    /// Creates a new `Declination` instance from a raw mount angle without clamping.
    /// This should only be used internally by the pointing model.
    fn new_raw(raw_degrees: f64) -> Self {
        Self { degrees: raw_degrees }
    }

    // `new_from_degrees` is redundant if `new` handles normalization.
    // If you intend `new` to be a raw constructor and `new_from_degrees` to normalize,
    // then rename `new` to `new_raw` and `new_from_degrees` becomes `new`.
    // Sticking with `new` normalizing is usually cleaner for coordinate types.
    // So, `pub fn new_from_degrees(degrees: f64) -> Self { Self::new(degrees) }` is not needed.

    /// Creates a new `Declination` value from a tuple of degrees, minutes, and seconds.
    pub fn new_from_dms(degrees: i32, minutes: i32, seconds: f64) -> Self {
        let sign = if degrees < 0 || degrees.to_string().starts_with('-') { -1.0 } else { 1.0 };
        let value = sign * (degrees.abs() as f64 + (minutes as f64 / 60.0) + (seconds / 3600.0));
        Declination::new(value) // Call the `new` constructor, which will normalize.
    }

    /// Creates a new `Declination` value from a string representing a declination
    /// in a format like "+12:34:56.78" or "-05:00:00.00".
    /// Returns a `Result` for robust parsing.
    pub fn new_from_dms_string(dms_string: &str) -> Result<Self, ParseDecError> {
        let parts: Vec<&str> = dms_string.split(':').collect();
        if parts.len() != 3 {
            // Error for incorrect format (e.g., missing parts)
            return Err(ParseDecError::InvalidFormat);
        }

        // Use .map_err() to convert ParseIntError to ParseFloatError, as discussed for RA
        let degrees = parts[0].parse::<i32>().map_err(|_| ParseDecError::InvalidDegrees)?;
        let minutes = parts[1].parse::<i32>().map_err(|_| ParseDecError::InvalidMinutes)?;
        let seconds = parts[2].parse::<f64>().map_err(|_| ParseDecError::InvalidSeconds)?;

        Ok(Declination::new_from_dms(degrees, minutes, seconds))
    }

    /// Creates a new `Declination` value from motor steps.
    /// Requires an instance of `AxisDrivetrain` to calculate degrees per step.
    pub fn new_from_motor_steps(steps: i32, drivetrain: &AxisDrivetrain) -> Self {
        Self::new(drivetrain.steps_to_degrees(steps))
    }

    /// Creates a `Declination` from motor steps without normalization.
    pub fn new_from_motor_steps_unnormalized(steps: i32, drivetrain: &AxisDrivetrain) -> Self {
        let raw_degrees = drivetrain.steps_to_degrees(steps);
        Self::new_raw(raw_degrees)
    }

    /// Calculates the angular difference between this Declination and a reference Declination.
    /// The result is a `Declination` value representing the difference in degrees.
    /// Note: This doesn't inherently normalize as it represents a delta, not a coordinate.
    pub fn delta_from(&self, reference: Declination) -> Declination {
        Declination { degrees: self.degrees - reference.degrees }
    }

    /// Normalizes the declination value by clamping it to be between -90 and 90 degrees.
    /// Values outside this range will be clipped to the nearest pole.
    pub fn normalize_clamp_old(&self) -> Self {
        Declination{ degrees: self.degrees.clamp(-90.0, 90.0) }
    }

    pub fn normalize_clamp(&self) -> Self {
            let mut degrees = self.degrees;
            // Clamp only if exactly ±90° or beyond
            //if degrees > 90.0 {
            //    degrees = 90.0;
            //} else if degrees < -90.0 {
            //    degrees = -90.0;
            //}
            degrees = degrees.clamp(-90.0, 90.0);
        Declination { degrees }
    }

    /// Normalizes the declination value by wrapping it around the poles.
    /// For example, 95 degrees becomes 85 degrees (on the other side of the pole).
    /// 100 degrees becomes 80 degrees (past North Pole, now 80 degrees from North Pole on other side).
    /// -95 degrees becomes -85 degrees (past South Pole, now 85 degrees from South Pole on other side).
    pub fn normalize_wrap(&self) -> Self {
        let mut value = self.degrees;

        // Ensure value is between -180 and +180 first for consistent wrapping behavior
        // value = value.rem_euclid(360.0);
        //value = value % 360.0;
        value %= 360.0;
        if value > 180.0 {
            value -= 360.0;
        } else if value <= -180.0 {
            value += 360.0;
        }

        if value > 90.0 {
            Declination { degrees: 180.0 - value } // Example: 100 -> 80
        } else if value < -90.0 {
            Declination { degrees: -180.0 - value } // Example: -100 -> -80
        } else {
            Declination { degrees: value } // Already within -90 to 90
        }
    }

    /// Returns the declination value in degrees.
    pub fn value(&self) -> f64 {
        self.degrees
    }

    /// Returns the declination value in degrees. (Alias for `value()`)
    pub fn as_degrees(&self) -> f64 {
        self.degrees
    }

    /// Returns the declination value as a tuple of degrees, minutes, and seconds.
    pub fn as_dms(&self) -> (i32, i32, f64) {
        let value = self.value();
        let sign = value.signum();
        let abs_value = self.degrees.abs();

        let degrees = (abs_value.trunc() * sign) as i32;
        let minutes = (abs_value.fract() * 60.0).trunc() as i32;
        let seconds = (abs_value.fract() * 60.0).fract() * 60.0;

        (degrees, minutes, seconds)
    }

    /// Returns the declination value as a string in the format "DD:MM:SS.SS"
    pub fn as_dms_string(&self) -> String {
        let (degrees, minutes, seconds) = self.as_dms();
        // Ensure degrees is formatted with its sign, and zero-padded if positive.
        // e.g., +00, +12, -05
        let formatted_degrees = format!("{:03}", degrees); // This handles the sign for you.
        format!("{:+03}:{:02}:{:05.2}", formatted_degrees, minutes.abs(), seconds.abs())
    }

    /// Returns the declination value as motor steps.
    /// Requires an instance of `AxisDrivetrain` to calculate steps per degree.
    pub fn as_motor_steps(&self, drivetrain: &AxisDrivetrain) -> i32 {
        drivetrain.degrees_to_steps(self.degrees)
    }
}

// Implement arithmetic traits for Declination
// These operations do not implicitly normalize, allowing for calculations
// where the result might temporarily be outside the [-90, 90] range.
// Call `normalize_clamp()` or `normalize_wrap()` explicitly after arithmetic
// if you need a valid celestial coordinate.
impl std::ops::Add for Declination {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { degrees: self.degrees + rhs.degrees }
    }
}
impl std::ops::Sub for Declination {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { degrees: self.degrees - rhs.degrees }
    }
}
impl std::ops::AddAssign for Declination {
    fn add_assign(&mut self, rhs: Self) {
        self.degrees += rhs.degrees;
    }
}
impl std::ops::SubAssign for Declination {
    fn sub_assign(&mut self, rhs: Self) {
        self.degrees -= rhs.degrees;
    }
}

