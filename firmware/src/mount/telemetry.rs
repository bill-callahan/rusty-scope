/// /src/mount/telemetry.rs
/// 
/// this does all the calculating and updating of the coordinates.
/// 

use esp_idf_svc::hal::task::CriticalSection;
use log::info;
use anyhow::Result;

use crate::mount::{app_state::PierSide, commands::Command, Altitude, Azimuth, Declination, RightAscension, RustyScope};



impl<'a> RustyScope<'a> {
    /// Updates the telemetry
    pub fn update_telemetry(&self) -> Result<()> {
        info!("Commanded to update telemetry...");
        let cmd = Command::UpdateTelemetry;
        self.send_command(cmd);
        Ok(())
    }
    pub fn execute_update_telemetry(&self) -> Result<()> {
        //info!("Executing telemetry update...");
        
        

        // Get all necessary live data
        // let planner_state = self.system.planner.lock().unwrap();
        let _cs = CriticalSection::new();
        let planner_state = unsafe { &*self.system.planner.0 };
        // let time_state = self.system.time.get_state();
        let current_lst = self.system.time.get_lst();
        let site_info = self.app_state.lock().unwrap().site_info.clone();

        // --- Determine Side of Pier (Hemisphere Agnostic and Correct) ---
        let physical_ra_mount_degrees = self.ra_drivetrain.steps_to_degrees(
            planner_state.current_position_steps()[0]
        );
        let physical_dec_mount_degrees = self.dec_drivetrain.steps_to_degrees(
            planner_state.current_position_steps()[1]
        );

        // Get the configured "home" pole for this hemisphere.
        let home_pole_dec = self.dec_drivetrain.home_offset_degrees; // e.g., +90.0 or -90.0
    
        let side_of_pier: PierSide;

        // --- The Universal Rule ---
        // First, check for the exceptional "pole flip" case.
        if (home_pole_dec > 0.0 && physical_dec_mount_degrees > 90.0) ||  // Flipped past North Pole
           (home_pole_dec < 0.0 && physical_dec_mount_degrees < -90.0) { // Flipped past South Pole
        
            side_of_pier = PierSide::WestFlipped;

        } else {
            // If not pole-flipped, use the standard Hour Angle rule.
            let ha_hours = physical_ra_mount_degrees / 15.0;
            if ha_hours > 0.0 {
                side_of_pier = PierSide::West;
            } else {
                side_of_pier = PierSide::East;
            }
        }

        // Do all the calculations....
        let (ra, dec) = Self::get_radec(&self, planner_state, current_lst, side_of_pier);
        let (alt, az) = Self::equatorial_to_horizontal(&ra, &dec, current_lst, site_info.latitude.as_degrees());

        // update that app_state data fields
        let mut app_state = self.app_state.lock().unwrap();
        app_state.current_ra = ra;
        app_state.current_dec = dec;
        app_state.current_az = az;
        app_state.current_alt = alt;
        app_state.side_of_pier = side_of_pier; // Store the current side of pier
        //info!("Telemetry updated successfully.");
        //log::info!("[TELEMETRY] Updating current RA/Dec. Target is NOT being changed.");
        Ok(())
    }

    /// Calculates the current Right Ascension and Declination based on the
    /// mount's physical step position and the current sidereal time.
    fn get_radec(
        &self,
        //planner_state: &std::sync::MutexGuard<crate::stepper::MotionPlanner>,
        planner_state: &crate::stepper::MotionPlanner,
        //time_state: &crate::time::TimeState,
        lst_hours: f64,
        sideofpier: PierSide,
    ) -> (RightAscension, Declination) {
        // 1. Get current RAW PHYSICAL motor steps from the planner state.
        let physical_ra_steps = planner_state.current_position_steps()[0];
        let physical_dec_steps = planner_state.current_position_steps()[1];
    
        // 2. Get the sync offset.
        let sync_offset = planner_state.sync_steps();

        // 3. Convert PHYSICAL steps to LOGICAL steps.
        // logical_pos = physical_pos - sync_offset
        let logical_ra_steps = physical_ra_steps - sync_offset[0];
        let logical_dec_steps = physical_dec_steps - sync_offset[1];

        // 4. Convert LOGICAL steps to celestial coordinates.
        let mut ra = RightAscension::new_from_motor_steps(logical_ra_steps, lst_hours, &self.ra_drivetrain);
        //let dec = Declination::new_from_motor_steps(logical_dec_steps, &self.dec_drivetrain);
        if sideofpier == PierSide::WestFlipped {
            // We are on the flipped side, so we must add 12 hours to the RA.
            ra = RightAscension::new_from_hours(ra.total_hours() + 12.0);
        }

        let dec = Declination::new_from_motor_steps_unnormalized(logical_dec_steps, &self.dec_drivetrain).normalize_wrap();

        (ra, dec)
    }

    /// Converts Equatorial (RA/Dec) coordinates to local Horizontal (Alt/Az) coordinates.
    /// This version uses a numerically stable formula that works correctly at the poles.
    fn equatorial_to_horizontal(
        ra: &RightAscension,
        dec: &Declination,
        lst_hours: f64,
        latitude_degrees: f64,
        ) -> (Altitude, Azimuth) {

        let lat_rad = latitude_degrees.to_radians();
        let dec_rad = dec.as_degrees().to_radians();
        // Hour Angle in radians. HA = LST - RA
        let ha_rad = (lst_hours - ra.total_hours()) * (15.0_f64).to_radians();

        // --- Formula for Altitude ---
        let sin_alt = lat_rad.sin() * dec_rad.sin() + lat_rad.cos() * dec_rad.cos() * ha_rad.cos();
        let alt_rad = sin_alt.clamp(-1.0,1.0).asin();
        let altitude = Altitude::new(alt_rad.to_degrees());

        // --- Numerically Stable Formula for Azimuth ---
        let y = ha_rad.sin();
        let x = ha_rad.cos() * lat_rad.sin() - dec_rad.tan() * lat_rad.cos();
    
       // The result of atan2 is in [-PI, PI]. We convert it to [0, 360] degrees.
        // Azimuth is measured from North=0, East=90, South=180, West=270.
        let _az_rad = y.atan2(x);
    
        // Let's re-evaluate the atan2 formula. The one from Wikipedia is:
        // y = -sin(HA)
        // x = tan(Dec)*cos(Lat) - sin(Lat)*cos(HA)
        // Az = atan2(y, x)
    
        // Let's use the formulas from the US Naval Observatory, which are very robust.
        let y_az = ha_rad.sin() * dec_rad.cos();
        let x_az = ha_rad.cos() * dec_rad.cos() * lat_rad.sin() - dec_rad.sin() * lat_rad.cos();
    
        // atan2 gives a result from -PI to +PI. We need to convert to 0-360 degrees.
        let mut az_degrees = y_az.atan2(x_az).to_degrees();

        // The result from this formula is often measured from South.
        // To convert to the standard North=0, East=90:
        az_degrees += 180.0;
    
        let azimuth = Azimuth::new(az_degrees); // `new` will normalize it to the [0, 360) range.

        (altitude, azimuth)
    }

    /// Converts local Horizontal (Alt/Az) coordinates to Equatorial (RA/Dec) coordinates.
    /// This is the inverse of the `equatorial_to_horizontal` function.
    pub fn horizontal_to_equatorial(
        alt: &Altitude,
        az: &Azimuth,
        lst_hours: f64,
        latitude_degrees: f64,
    ) -> (RightAscension, Declination) {
        let lat_rad = latitude_degrees.to_radians();
        let alt_rad = alt.as_degrees().to_radians();
        let az_rad = az.as_degrees().to_radians();

        // --- Formula for Declination ---
        let sin_dec = lat_rad.sin() * alt_rad.sin() + lat_rad.cos() * alt_rad.cos() * az_rad.cos();
        let dec_rad = sin_dec.asin();
        let dec = Declination::new(dec_rad.to_degrees());

        // --- Numerically Stable Formula for Hour Angle ---
        let y_ha = -az_rad.sin() * alt_rad.cos();
        let x_ha = alt_rad.sin() * lat_rad.sin() - lat_rad.cos() * lat_rad.sin() * az_rad.cos();
        let ha_rad = y_ha.atan2(x_ha);

        // Convert Hour Angle from radians to hours
        let ha_hours = ha_rad.to_degrees() / 15.0;

        // --- Formula for Right Ascension ---
        // RA = LST - HA
        let ra_hours = lst_hours - ha_hours;
        let ra = RightAscension::new_from_hours(ra_hours);

        (ra, dec)
    }
    
}