

// src/mount/app_state.rs

use std::sync::{Arc, Mutex};

use hifitime::Epoch;

use crate::mount::{site::{Site}, Altitude, Azimuth, Declination, Latitude, Longitude, RightAscension};

// A convenient type alias for the thread-safe version.
pub type SharedAppState = Arc<Mutex<RustyScopeState>>;

// This will remember if we should resume tracking after a focus move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreFocusState {
    Idle,
    WasTracking,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PierSide {
    East, // Or "Normal", "West looking East"
    West, // Or "Flipped", "East looking West"
    WestFlipped,
    Unknown = -1,
}

/// A snapshot of all relevant telescope telemetry data at a specific moment.
/// This struct is cheap to clone and safe to pass between threads.
#[derive(Debug, Clone)]
pub struct RustyScopeState {
    // Celestial Coordinates
    pub current_ra: RightAscension,
    pub current_dec: Declination,
    pub target_ra: RightAscension,
    pub target_dec: Declination,
    
    // Local Coordinates
    pub current_alt: Altitude,
    pub current_az: Azimuth,

    // Side of pier
    pub side_of_pier: PierSide,
    
    // Time and Site
    pub sidereal: f64,
    pub utc_time: Option<hifitime::Epoch>,
    pub site_info: Site,

    // Mount Status
    pub is_slewing: bool,
    pub is_tracking: bool,
    pub is_guiding: bool,
    pub is_parked: bool,
    pub at_park: bool,
    pub at_home: bool,
    pub pre_focus_state: PreFocusState,
    pub tracking_enabled: bool,

    // Connection -status
    pub focconnecting: bool,
    pub focconnected: bool,
    pub telconnecting: bool,
    pub telconnected: bool,

    // various rates
    pub ra_rate: f32,
    pub dec_rate: f32,
    pub guide_dec_rate: f32,
    pub guide_ra_rate: f32,
    pub tracking_rate: i32,
    // Stores the timestamp of the last time a non-sidereal track was started.
    pub last_non_sidereal_track_time: Option<Epoch>,
    
    // Raw Motor Data (useful for diagnostics)
    pub ra_motor_steps: i32,
    pub dec_motor_steps: i32,
    pub focuser_position: i32,
}

impl Default for RustyScopeState {
    fn default() -> Self {
        Self {
            current_ra: RightAscension { seconds: 0.0 },
            current_dec: Declination { degrees: 0.0 },
            target_ra: RightAscension { seconds: 0.0 },
            target_dec: Declination { degrees: 0.0 },
            current_alt: Altitude { degrees: 0.0 },
            current_az: Azimuth { degrees: 0.0 },
            sidereal: 0.0,
            utc_time: None,
            site_info: Site {
                name: "NotSet".to_string(),
                latitude: Latitude { degrees: 0.0 },
                longitude: Longitude { degrees: 0.0 },
                elevation: 0.0,
                tz_offset_hours: 0,
            },
            side_of_pier: PierSide::East,
            is_slewing: false,
            is_tracking: false,
            is_guiding: false,
            is_parked: false,
            at_park: false,
            at_home: false,
            tracking_enabled: true,
            pre_focus_state: PreFocusState::Idle,
            focconnecting: false,
            focconnected: false,
            telconnecting: false,
            telconnected: false,
            ra_motor_steps: 0,
            dec_motor_steps: 0,
            focuser_position: 0,
            ra_rate: 1.0,
            dec_rate: 1.0,
            guide_dec_rate: 0.0,
            guide_ra_rate: 0.0,
            tracking_rate: 0,
            last_non_sidereal_track_time: None,
        }
    }
}