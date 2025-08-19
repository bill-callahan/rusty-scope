// src/mount/site.rs




use crate::mount::{Latitude, Longitude};
/// A site is a location on Earth, with a name, latitude, longitude, elevation, and timezone.

#[derive(Debug, Clone)]
pub struct Site {
    pub name: String,
    pub latitude: Latitude,
    pub longitude: Longitude,
    pub elevation: f64,
    pub tz_offset_hours: i32,
}

impl Site {
    /// Create a new site with the given name, latitude, longitude, and elevation.
    /// The timezone offset is computed from the longitude.
    pub fn new(name: String, latitude: Latitude, longitude: Longitude, elevation: f64) -> Self {
        let tz_offset_hours = (longitude.as_degrees() / 15.0).round() as i32;
        Self {
            name,
            latitude,
            longitude,
            elevation,
            tz_offset_hours,
        }
    }
    pub fn print_summary(&self) {
        
        println!("_____________ Site details ____________________________________________");
        println!("|  Site: {} - LAT {}  LON {}            |", self.name, self.latitude.as_wgs84_string(), self.longitude.as_wgs84_string()) ;
        println!("|_____________________________________________________________________|");
        println!("|  Elevation: {} meters                                             |", self.elevation);
        println!("|  Timezone offset: {} hours from UTC                                 |", self.tz_offset_hours);
        println!("|  Home: {:?} Celestial Pole                                         |", self.latitude.hemisphere());
        println!("|_____________________________________________________________________|");

    }
}

impl Default for Site {
    fn default() -> Self {
        let name = "GPS".to_string();
        let latitude = Latitude::new(-37.0);
        let longitude = Longitude::new(145.0);
        let elevation = 0.0;

        let tz_offset_hours = (longitude.as_degrees() / 15.0).round() as i32;
        
        Site {
            name,
            latitude,
            longitude,
            elevation,
            tz_offset_hours,
        }
    }
}