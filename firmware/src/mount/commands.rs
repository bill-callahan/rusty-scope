// src/mount/commands.rs

use crate::mount::{tracking::TrackingRateType, Declination, RightAscension};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuideDirection {
    North,
    South,
    East,
    West,
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    // Mount commands
    MoveFocuser { position: i32 },
    PulseGuide { direction: GuideDirection, duration_ms: u32 },
    SyncToCoordinates { ra: RightAscension, dec: Declination },
    MoveAxis { axis: u8, rate_dps: f32 },
    Park,
    Home,
    CalibrateTracking,
    StartTracking,
    StopTracking,
    SetTracking { enabled: bool, rate: TrackingRateType },
    StopMotion,
    UpdateTelemetry,
    SlewToTarget,
    None,
}