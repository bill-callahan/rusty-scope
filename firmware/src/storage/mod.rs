// src/storage/mod.rs

pub mod spiffs;
//pub mod sdcard;

use self::spiffs::SpiffsManager;
//use self::sdcard::SdCardManager;
use esp_idf_svc::sys::EspError;
use log::info;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Default)]
pub struct DeviceConfig {
    pub configured: bool,
    pub motorx_spr: u32,
    pub motorx_microsteps: u32,
    pub motorx_gratio: f64,
    pub motorx_offset: f64,
    pub motory_spr: u32,
    pub motory_microsteps: u32,
    pub motory_gratio: f64,
    pub motory_offset: f64,
    pub motorz_spr: u32,
    pub motorz_microsteps: u32,
    pub motorz_gratio: f64,
    pub motorz_offset: f64,
    pub last_ra_steps: i32,
    pub last_dec_steps: i32,
    pub site_longitude: f64,
    pub site_latitude: f64,
    pub guide_ra_rate_dps: f32,
    pub guide_dec_rate_dps: f32,
    pub default_tracking_rate: i32,
    pub move_axis_ra_rate_dps: f32,
    pub move_axis_dec_rate_dps: f32,
    /// A correction factor to apply to the sidereal tracking rate.
    /// Default is 1.0 (no correction).
    /// A value of 1.0001 would speed up tracking slightly.
    /// A value of 0.9999 would slow down tracking slightly.
    #[serde(default = "default_tracking_correction")]
    pub tracking_correction_factor: f64,
    #[serde(default = "default_tracking_calibration_factor")]
    pub tracking_calibration_factor: f64,
}

// Helper function for serde to provide the default value.
fn default_tracking_correction() -> f64 {
    1.0
}
fn default_tracking_calibration_factor() -> f64 {
    1.005
}

pub struct StorageManager {
    // We need to use this field, so let's remove the underscore
    spiffs: SpiffsManager,
}

impl StorageManager {
    const CONFIG_PATH: &'static str = "/spiffs/config.json";

    pub fn new() -> Result<Self, esp_idf_svc::sys::EspError> {
    // The SpiffsManager::new with `format_if_mount_failed` handles everything.
    let spiffs = SpiffsManager::new("storage")?;
    
    // We still need to create the base directory, as formatting doesn't do that.
    // But we don't need the complex recovery logic anymore.
    let _ = fs::create_dir_all("/spiffs/")
        .map_err(|e| EspError::from(e.raw_os_error().unwrap_or(0))); // propagate error as EspError

    Ok(Self { spiffs })
}
    
    pub fn save_config(&self, config: &DeviceConfig) -> anyhow::Result<()> {
        let json_string = serde_json::to_string(config)?;
        self.spiffs.write_file(Self::CONFIG_PATH, &json_string)?;
        Ok(())
    }

    pub fn load_config(&self) -> anyhow::Result<DeviceConfig> {
        if self.spiffs.file_exists(Self::CONFIG_PATH) {
            let json_string = self.spiffs.read_file(Self::CONFIG_PATH)?;
            let config: DeviceConfig = serde_json::from_str(&json_string)?;
            Ok(config)
        } else {
            Ok(DeviceConfig::default())
        }
    }
    pub fn delete_config(&self) -> anyhow::Result<()> {
        info!("Deleting configuration file at '{}'", Self::CONFIG_PATH);
        self.spiffs.delete_file(Self::CONFIG_PATH)?;
        Ok(())
    }
    pub fn new_config(&self) -> anyhow::Result<DeviceConfig> {
        let mut config = DeviceConfig::default();
        config.configured = false;
        self.save_config(&config)?;
        Ok(config)
    }
    pub fn set_config(&self) -> anyhow::Result<DeviceConfig> {
        let mut rsconfig = DeviceConfig::default();
        rsconfig.motorx_spr = 200;
        rsconfig.motorx_microsteps = 128;
        rsconfig.motorx_gratio = 100.0;
        rsconfig.motorx_offset = 0.0;
        rsconfig.motory_spr = 200;
        rsconfig.motory_microsteps = 128;
        rsconfig.motory_gratio = 100.0;
        rsconfig.motory_offset = 0.0;
        rsconfig.motorz_spr = 200;
        rsconfig.motorz_microsteps = 128;
        rsconfig.motorz_gratio = 1.0;
        rsconfig.motorz_offset = 0.0;
        rsconfig.configured = true;

        // Set a default guide rate of 0.5x sidereal (~7.5 arcsec/sec).
        // The Alpaca property is in degrees/sec.
        const HALF_SIDEREAL_DPS: f32 = 7.52 / 3600.0;
        rsconfig.guide_ra_rate_dps = HALF_SIDEREAL_DPS;
        rsconfig.guide_dec_rate_dps = HALF_SIDEREAL_DPS;

        // Default to Sidereal tracking.
        rsconfig.default_tracking_rate = 0; // 0 = Sidereal
    
        // Set a default manual move rate of 1.0 deg/sec.
        rsconfig.move_axis_ra_rate_dps = 1.0;
        rsconfig.move_axis_dec_rate_dps = 1.0;

        // Set a default calibration factor. 1.00002 is a good starting point.
        // This makes tracking about 1.7 seconds faster over a 24-hour period.
        // rsconfig.tracking_calibration_factor = 1.00002;
        // Let's try a more assertive calibration factor.
        // The math suggests ~1.0068. Let's try 1.005 as a starting point.
        rsconfig.tracking_calibration_factor = 1.005;

        rsconfig.configured = true;

        self.save_config(&rsconfig)?;
        Ok(rsconfig)
    }
}

