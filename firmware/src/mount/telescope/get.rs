// src/mount/telescope/get.rs
use std::f64::consts::PI;
use std::sync::Arc;


use crate::mount::alpacaserver::ApiValue;
use crate::mount::RustyScope;

// Define the maximum manual slew rates in degrees per second.
// These should be chosen based on testing your physical hardware.
const MAX_RA_SLEW_RATE_DPS: f32 = 2.5;
const MAX_DEC_SLEW_RATE_DPS: f32 = 2.5;

// The focuser does not support variable rate moves via this command.
const MAX_FOCUS_SLEW_RATE_DPS: f32 = 0.0;

pub fn handle_telescope_get(scope: Arc<RustyScope<'static>>, endpoint: &str, axis: Option<i32>) -> ApiValue {
    // spawn( move || {
        //println!("handle_telescope_get Thread: {:?} on core {:?}", std::thread::current().id(), esp_idf_svc::hal::cpu::core());
        //println!("Free heap: {} bytes", unsafe { esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_DEFAULT) });
    let data = scope.app_state.lock().unwrap();
    match endpoint {
        // alignmentmode - The alignment mode of the telescope (AltAz(0), Polar(1), GermanPolar(2))
        "alignmentmode" => ApiValue::I32(1),
        // aperturearea - The area of the telescope's aperture, taking into account any obstructions (square meters)
        "aperturearea" => ApiValue::F64(PI * (0.061 / 2.0) * (0.061 / 2.0)),
        // aperturediameter - The diameter of the telescope's aperture (meters)
        "aperturediameter" => ApiValue::F64(0.061),
        // equatorilsystem - The equatorial coordinate system used by the telescope (TopoCentric(1), J2000(2), J2050(3), B1950(4))
        "equatorialsystem" => ApiValue::I32(2),
        // focallength - The focal length of the telescope's primary mirror (meters)
        "focallength" => ApiValue::F32(0.337),
        // utcdate - The UTC date and time (seconds since 1970-01-01T00:00:00Z)
        // "utcdate" => ApiValue::String(scope.system.time.get_utc_string()),
        // utcdate - The UTC date and time in ISO 8601-1 format.
        "utcdate" => ApiValue::String(scope.system.time.get_utc_string_alpaca()),
        "siderealtime" => ApiValue::F64(scope.system.time.get_lst()),
        "connected" => ApiValue::Bool(data.telconnected),
        "connecting" => ApiValue::Bool(data.telconnecting),
        "description" => ApiValue::String("Rusty-scope Telescope".to_string()),
        "driverinfo" => ApiValue::String("ESP32 Alpaca Server".to_string()),
        "driverversion" => ApiValue::String(env!("CARGO_PKG_VERSION").to_string()),
        "interfaceversion" => ApiValue::I32(4),
        "name" => ApiValue::String("Rusty-scope".to_string()),
        "supportedactions" => ApiValue::List(vec!(
            ApiValue::String("DeclarePolarAlignment".to_string())
        )),
        "athome" => ApiValue::Bool(data.at_home),
        "atpark" => ApiValue::Bool(data.at_park),
        "rightascension" => ApiValue::F64(data.current_ra.total_hours()),
        "declination" => ApiValue::F64(data.current_dec.as_degrees()),
        "altitude" => ApiValue::F64(data.current_alt.as_degrees()),
        "azimuth" => ApiValue::F64(data.current_az.as_degrees()),
        "sitelatitude" => ApiValue::F64(data.site_info.latitude.as_degrees()),
        "sitelongitude" => ApiValue::F64(data.site_info.longitude.as_degrees()),
        "siteelevation" => ApiValue::F64(data.site_info.elevation),
        "targetrightascension" => ApiValue::F64(data.target_ra.total_hours()),
        "targetdeclination" => ApiValue::F64(data.target_dec.as_degrees()),
        "slewsettletime" => ApiValue::I32(3),
        "sideofpier" => ApiValue::I32(data.side_of_pier as i32),
        "tracking" => ApiValue::Bool(data.is_tracking),
        "slewing" => ApiValue::Bool(data.is_slewing),
        "ispulseguiding" => ApiValue::Bool(data.is_guiding),
        
        "trackingrate" => ApiValue::I32(data.tracking_rate),
        "declinationrate" => ApiValue::F32(data.dec_rate),
        "rightascensionrate" => ApiValue::F32(data.ra_rate),
        "guideratedeclination" => ApiValue::F32(data.guide_dec_rate),
        "guideraterightascension" => ApiValue::F32(data.guide_ra_rate),
        "cansetrightascensionrate" => ApiValue::Bool(true),
        "cansetdeclinationrate" => ApiValue::Bool(true),
        "cansync" => ApiValue::Bool(true),
        "canslew" => ApiValue::Bool(true),
        "canslewasync" => ApiValue::Bool(true),
        "canslewaltaz" => ApiValue::Bool(false),
        "canslewaltazasync" => ApiValue::Bool(false),
        "cansyncaltaz" => ApiValue::Bool(false),
        "cansettracking" => ApiValue::Bool(true),
        "cansetguiding" => ApiValue::Bool(true),
        "cansetpark" => ApiValue::Bool(true),
        "cansettime" => ApiValue::Bool(false),
        "canfindhome" => ApiValue::Bool(true),
        "canpark" => ApiValue::Bool(true),
        "canunpark" => ApiValue::Bool(true),
        "cansetguiderates" => ApiValue::Bool(true),
        "cansetpierside" => ApiValue::Bool(false),
        "canpulseguide" => ApiValue::Bool(true),
        "doesrefraction" => ApiValue::Bool(false),
        "axisrates" => {
            // Helper function to create the standard response structure.
            fn create_rate_response(max_rate: f32) -> ApiValue {
                ApiValue::List(vec![
                    ApiValue::KeyPairs(vec![
                        ("Maximum".to_string(), ApiValue::F32(max_rate)),
                        ("Minimum".to_string(), ApiValue::F32(0.0)),
                    ])
                ])
            }
            match axis { 
                Some(0) => create_rate_response(MAX_RA_SLEW_RATE_DPS),    // Primary (RA) axis
                Some(1) => create_rate_response(MAX_DEC_SLEW_RATE_DPS),   // Secondary (Dec) axis
                Some(2) => create_rate_response(MAX_FOCUS_SLEW_RATE_DPS), // Tertiary (Focuser) axis
                _ => ApiValue::Null, // Invalid axis number
            }
        },
        // "trackingrates" => ApiValue::ArrayI(vec!(0,1,2),),  // 0 = Sidereal, 1=Lunar, 2=Solar, 3=King
        "trackingrates" => ApiValue::ArrayI(vec![
            0, // Sidereal - Fully supported
            1, // Lunar - Supported with approximation
            2, // Solar - Fully supported
        ]),
        "canmoveaxis" => {
            match axis {
                Some(0) => ApiValue::Bool(true),
                Some(1) => ApiValue::Bool(true),
                Some(2) => ApiValue::Bool(false),
                _ => ApiValue::Bool(false),
                }
            },
        "devicestate" => ApiValue::Array(vec!(
            ("Altitude".to_string(), ApiValue::F64(data.current_alt.as_degrees())),
            ("Azimuth".to_string(), ApiValue::F64(data.current_az.as_degrees())),
            ("Declination".to_string(), ApiValue::F64(data.current_dec.as_degrees())),
            ("RightAscension".to_string(), ApiValue::F64(data.current_ra.total_hours())),
            ("SiteLatitude".to_string(), ApiValue::F64(data.site_info.latitude.as_degrees())),
            ("SiteLongitude".to_string(), ApiValue::F64(data.site_info.longitude.as_degrees())),
            ("SiteElevation".to_string(), ApiValue::F64(data.site_info.elevation)),
            ("AtHome".to_string(), ApiValue::Bool(data.at_home)),
            ("AtPark".to_string(), ApiValue::Bool(data.at_park)),
            ("IsPulseGuiding".to_string(), ApiValue::Bool(data.is_guiding)),
            ("SideOfPier".to_string(), ApiValue::I32(-1)),
            ("SiderealTime".to_string(), ApiValue::F64(scope.system.time.get_lst())),
            ("Slewing".to_string(), ApiValue::Bool(data.is_slewing)),
            ("Tracking".to_string(), ApiValue::Bool(data.is_tracking)),
            ("UTCDate".to_string(), ApiValue::String(scope.system.time.get_utc_string_alpaca())),
            
            
        )),
                
        
        _ => ApiValue::Null,
    }
//});
}