
// src/mount/latlon.rs



// A custom error type for parsing makes the code much cleaner.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    InvalidFormat,
    InvalidComponent(String),
}


/// Represents a latitude value.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)] // Added Copy, PartialEq, PartialOrd
pub struct Latitude {
    pub degrees: f64,
}

/// Represents the hemisphere of a latitude value.
#[derive(Debug, Clone, Copy, PartialEq)] // Added Clone, Copy, PartialEq
pub enum LatHemisphere {
    North = 1,
    South = -1,
}

impl Latitude {
    /// Creates a new `Latitude` value from a floating-point number representing the
    /// latitude in degrees.
    /// This constructor normalizes the value to be between -90 and 90 degrees.
    pub fn new(degrees: f64) -> Self {
        Self { degrees: degrees.clamp(-90.0, 90.0) }
    }

    /// Creates a new `Latitude` value from a tuple of degrees, minutes, and seconds.
    pub fn new_from_dms(degrees: i32, minutes: i32, seconds: f64) -> Self {
        let sign = if degrees < 0 { -1.0 } else { 1.0 };
        // Minutes and seconds should always be positive when converting from DMS
        let total_degrees =  degrees.abs() as f64 + (minutes as f64 / 60.0) + (seconds / 3600.0);
        Self::new(total_degrees * sign)
    }

    /// Creates a new `Latitude` value from a string representing a latitude in a string like "-37:45:67.89"
    /// Returns a `Result` for robust parsing.
    pub fn new_from_dms_string(dms_string: &str) -> Result<Self, ParseError> {
        // No need for .to_string() here, &str is fine
        let parts: Vec<&str> = dms_string.split(':').collect();
        if parts.len() != 3 {
            return Err(ParseError::InvalidFormat);
        }

        let degrees = parts[0].parse::<i32>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;
        let minutes = parts[1].parse::<i32>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;
        let seconds = parts[2].parse::<f64>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;

        Ok(Self::new_from_dms(degrees, minutes, seconds))
    }

    /// Clamps the latitude value between -90 and 90 degrees.
    /// Values outside this range will be clipped to the nearest pole.
    pub fn normalize(&self) -> Self {
        let lat = self.degrees.clamp(-90.0, 90.0);
        Latitude { degrees: lat }
    }

    /// Returns the latitude value in degrees.
    pub fn as_degrees(&self) -> f64 {
        self.degrees
    }

    /// Returns the latitude value as a tuple of degrees, minutes, and seconds.
    /// The degrees component carries the sign, minutes and seconds are always positive.
    pub fn as_dms(&self) -> (i32, i32, f64) {
        let degrees_float = self.degrees;
        let degrees = degrees_float.trunc() as i32;
        let remainder_mins = (degrees_float.abs() - degrees.abs() as f64) * 60.0;
        let minutes = remainder_mins.trunc() as i32;
        let seconds = (remainder_mins - minutes as f64) * 60.0;

               
        (degrees, minutes, seconds)
    }

    /// Returns the latitude value as a string like "DD° MM' SS.SS\""
    /// A leading '+' will be added for positive degrees for clarity.
    pub fn as_dms_string(&self) -> String {
        let (degrees, minutes, seconds) = self.as_dms();
        // Use {:+} to force a leading plus sign for positive numbers
        format!("{:+03}° {:02}' {:05.2}\"", degrees, minutes, seconds)
    }

    /// Returns the latitude value as a string like "DD.MM.SS.SS North/South" (WGS84 style)
    pub fn as_wgs84_string(&self) -> String {
        let (degrees, minutes, seconds) = self.as_dms();
        let hemisphere_char = if self.degrees >= 0.0 { 'N' } else { 'S' };
        // Use abs for degrees as hemisphere char indicates sign
        format!("{}° {:02}' {:05.2}\" {}", degrees.abs(), minutes, seconds, hemisphere_char)
    }

    /// Returns the hemisphere of the latitude value.
    pub fn hemisphere(&self) -> LatHemisphere {
        if self.degrees >= 0.0 { LatHemisphere::North } else { LatHemisphere::South }
    }
}

/// Represents a longitude value.

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)] // Added Copy, PartialEq, PartialOrd
pub struct Longitude {
    pub degrees: f64,
}

/// Represents the hemisphere/direction of a longitude value.
#[derive(Debug, Clone, Copy, PartialEq)] // Added Clone, Copy, PartialEq
pub enum LonHemisphere {
    East = 1,
    West = -1,
}

impl Longitude {
    /// Creates a new `Longitude` value from a floating-point number representing the longitude in degrees.
    /// This constructor normalizes the value to be between -180 and 180 degrees.
    pub fn new(degrees: f64) -> Self {
        // Normalize to [0, 360) then shift to [-180, 180)
        let mut lon = degrees.rem_euclid(360.0);
        if lon >= 180.0 {
            lon -= 360.0;
        }
        Self { degrees: lon }
    }

    /// Creates a new `Longitude` value from a tuple of degrees, minutes, and seconds.
    pub fn new_from_dms(degrees: i32, minutes: i32, seconds: f64) -> Self {
        let sign = if degrees < 0 { -1.0 } else { 1.0 };
        // Minutes and seconds should always be positive when converting from DMS
        let total_degrees =  degrees.abs() as f64 + (minutes as f64 / 60.0) + (seconds / 3600.0);
        Self::new(total_degrees * sign)
    }

    /// Creates a new `Longitude` value from a string representing a longitude in a string like "-123:45:67.89"
    /// Returns a `Result` for robust parsing.
    pub fn new_from_dms_string(dms_string: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = dms_string.split(':').collect();
        if parts.len() != 3 {
            return Err(ParseError::InvalidFormat);
        }

        let degrees = parts[0].parse::<i32>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;
        let minutes = parts[1].parse::<i32>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;
        let seconds = parts[2].parse::<f64>().map_err(|e| ParseError::InvalidComponent(e.to_string()))?;

        Ok(Self::new_from_dms(degrees, minutes, seconds))
    }

    /// Normalizes the longitude value to be between -180 and 180 degrees.
    /// This function ensures the longitude value is within the valid range by
    /// wrapping it around 360 degrees, then adjusting to the [-180, 180] range.
    /// Example: 185.234 -> -174.766; -185.234 -> 174.766
    pub fn normalize(&self) -> Self {
        let mut lon = self.degrees;
        // Use rem_euclid to get a positive remainder [0, 360)
        lon = lon.rem_euclid(360.0);
        // Then adjust to [-180, 180)
        if lon > 180.0 {
            lon -= 360.0;
        }
        Longitude { degrees: lon }
    }

    /// Returns the longitude value in degrees.
    pub fn as_degrees(&self) -> f64 {
        self.degrees
    }

    /// Returns the longitude value as a tuple of degrees, minutes, and seconds.
    /// The degrees component carries the sign, minutes and seconds are always positive.
    pub fn as_dms(&self) -> (i32, i32, f64) {
        let degrees_float = self.degrees;
        let degrees = degrees_float.trunc() as i32;
        let remainder_mins = (degrees_float.abs() - degrees.abs() as f64) * 60.0;
        let minutes = remainder_mins.trunc() as i32;
        let seconds = (remainder_mins - minutes as f64) * 60.0;

               
        (degrees, minutes, seconds)
    }

    /// Returns the longitude value as a string like "DDD° MM' SS.SS\""
    /// A leading '+' will be added for positive degrees for clarity.
    pub fn as_dms_string(&self) -> String {
        let (degrees, minutes, seconds) = self.as_dms();
        // Use {:+} to force a leading plus sign for positive numbers
        format!("{:+03}° {:02}' {:05.2}\"", degrees, minutes, seconds)
    }

    /// Returns the longitude value as a string like "DDD.MM.SS.SS East/West" (WGS84 style)
    pub fn as_wgs84_string(&self) -> String {
        let (degrees, minutes, seconds) = self.as_dms();
        let hemisphere_char = if self.degrees >= 0.0 { 'E' } else { 'W' };
        
        // Use abs for degrees as hemisphere char indicates sign
        format!("{}° {:02}' {:05.2}\" {}", degrees.abs(), minutes, seconds, hemisphere_char)
    }

    /// Returns the hemisphere/direction of the longitude value.
    pub fn hemisphere(&self) -> LonHemisphere {
        if self.degrees >= 0.0 { LonHemisphere::East } else { LonHemisphere::West }
    }
}


