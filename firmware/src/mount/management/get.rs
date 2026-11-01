// src/mount/management/get.rs

use std::sync::Arc;
use serde_json::Value as JsonValue;

use esp_idf_svc::{hal::task::CriticalSection, sys::heap_caps_get_free_size};

use crate::mount::{alpacaserver::ApiValue, RustyScope};





pub fn handle_management_get(scope: Arc<RustyScope<'static>>, endpoint: &str) -> ApiValue {
    

    let data = scope.app_state.lock().unwrap();
    //let motors = scope.system.planner.lock().unwrap().current_position_steps();
    let motors = {
        let _cs = CriticalSection::new();
        let planner = unsafe { &*scope.system.planner.0 };
        planner.current_position_steps()
    };
    
    let mac = scope.system.mac_string.clone();

    let fuid = "RSFocuser-".to_string() + &mac;
    let tuid = "RSTelescope-".to_string() + &mac;
    
    match endpoint {
        "apiversions" => ApiValue::ArrayI(vec!(1)),
        "v1/description" => ApiValue::KeyPairs(vec!(
            ("ServerName".to_string(), ApiValue::String("RustyScope".to_string())),
            ("Manufacturer".to_string(), ApiValue::String("RustyScope".to_string())),
            ("ManufacturerVersion".to_string(), ApiValue::String(env!("CARGO_PKG_VERSION").to_string())),
            ("Location".to_string(), ApiValue::String("🌏Earth".to_string())),
        )),
        
        "v1/configureddevices" => ApiValue::List(vec![
            ApiValue::KeyPairs(vec![
                ("DeviceName".to_string(), ApiValue::String("RustyScope Focuser".to_string())),
                ("DeviceType".to_string(), ApiValue::String("Focuser".to_string())),
                ("DeviceNumber".to_string(), ApiValue::I32(0)),
                ("UniqueID".to_string(), ApiValue::String(fuid.clone())),
            ]),
            ApiValue::KeyPairs(vec![
                ("DeviceName".to_string(), ApiValue::String("RustyScope Telescope".to_string())),
                ("DeviceType".to_string(), ApiValue::String("Telescope".to_string())),
                ("DeviceNumber".to_string(), ApiValue::I32(0)),
                ("UniqueID".to_string(), ApiValue::String(tuid.clone())),
            ]),
        ]),
        "v1/state" => ApiValue::KeyPairs(vec![
            ("Altitude".to_string(), ApiValue::F64(data.current_alt.as_degrees())),
            ("Azimuth".to_string(), ApiValue::F64(data.current_az.as_degrees())),
            ("Declination".to_string(), ApiValue::F64(data.current_dec.as_degrees())),
            ("RightAscension".to_string(), ApiValue::F64(data.current_ra.total_hours())),
            ("TargetRA".to_string(), ApiValue::F64(data.target_ra.total_hours())),
            ("TargetDec".to_string(), ApiValue::F64(data.target_dec.as_degrees())),
            ("SiteLatitude".to_string(), ApiValue::F64(data.site_info.latitude.as_degrees())),
            ("SiteLongitude".to_string(), ApiValue::F64(data.site_info.longitude.as_degrees())),
            ("SiteElevation".to_string(), ApiValue::F64(data.site_info.elevation)),
            ("MotorX".to_string(), ApiValue::I32(motors[0])),
            ("MotorY".to_string(), ApiValue::I32(motors[1])),
            ("MotorZ".to_string(), ApiValue::I32(motors[2])),
            ("SiderealTime".to_string(), ApiValue::F64(scope.system.time.get_lst())),
            ("Slewing".to_string(), ApiValue::Bool(data.is_slewing)),
            ("Tracking".to_string(), ApiValue::Bool(data.is_tracking)),
            ("Guiding".to_string(), ApiValue::Bool(data.is_guiding)),
            ("UTCDate".to_string(), ApiValue::String(scope.system.time.get_utc_string())),
        ]),
        "v1/config" => {
            // Load the full, persistent DeviceConfig from storage.
            match scope.get_persistent_config() {
                Ok(config) => {
                    // Convert the DeviceConfig struct into a generic serde_json::Value
                    let json_val = serde_json::to_value(config).unwrap_or(JsonValue::Null);
                    // Then convert the serde_json::Value into our ApiValue
                    ApiValue::from(json_val)
                }
                Err(e) => ApiValue::KeyPairs(vec![
                    ("error".to_string(), ApiValue::String(format!("Failed to load config: {}", e)))
                ]),
            }
        },
        // --- ENDPOINT FOR LIVE STATUS (Read-Only) ---
        "v1/status" => {
            let free_heap = unsafe { heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_DEFAULT) };
            ApiValue::KeyPairs(vec![
                ("firmware_version".to_string(), ApiValue::String(env!("CARGO_PKG_VERSION").to_string())),
                ("ip_address".to_string(), ApiValue::String(scope.system.ipaddr.to_string())),
                ("mac_address".to_string(), ApiValue::String(scope.system.mac_string.clone())),
                ("free_heap_bytes".to_string(), ApiValue::I32(free_heap as i32)),
                // Uptime can be added here later
            ])
        },
        _ => ApiValue::Null,
    }
}

// --- New `From` implementation for ApiValue ---
// This is a powerful helper that lets us convert any `serde_json::Value`
// directly into our `ApiValue` enum. This is the cleanest way.
impl From<JsonValue> for ApiValue {
    fn from(value: JsonValue) -> Self {
        match value {
            JsonValue::Null => ApiValue::Null,
            JsonValue::Bool(b) => ApiValue::Bool(b),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ApiValue::I32(i as i32)
                } else if let Some(f) = n.as_f64() {
                    ApiValue::F64(f)
                } else {
                    ApiValue::Null
                }
            }
            JsonValue::String(s) => ApiValue::String(s),
            JsonValue::Array(arr) => {
                ApiValue::List(arr.into_iter().map(ApiValue::from).collect())
            }
            JsonValue::Object(map) => {
                let key_pairs = map.into_iter()
                    .map(|(k, v)| (k, ApiValue::from(v)))
                    .collect();
                ApiValue::KeyPairs(key_pairs)
            }
        }
    }
}
