// src/mount/focuser/get.rs
use std::sync::Arc;


use esp_idf_svc::hal::task::CriticalSection;

use crate::mount::alpacaserver::ApiValue;
use crate::mount::RustyScope;


pub fn handle_focuser_get(scope: Arc<RustyScope<'static>>, endpoint: &str) -> ApiValue {

    let (focuser_position, focuser_ismoving) = {
        let _cs = CriticalSection::new();
        let planner = unsafe { &*scope.system.planner.0 };
        (
            planner.current_position_steps()[2],
            planner.is_focuser_moving()
        )
    };
    let mut data = scope.app_state.lock().unwrap();
    //let motor_pos_steps = scope.system.planner.lock().unwrap().current_position_steps();
    //data.focuser_position = motor_pos_steps[2];
    data.focuser_position = focuser_position;
    //drop(motor_pos_steps);
    //let focuser_ismoving = scope.system.planner.lock().unwrap().is_busy();
    match endpoint {
        // absolute - True if the focuser is capable of absolute position; that is, being commanded to a specific step location.
        "absolute" => ApiValue::Bool(true),
        // ismoving - True if the focuser is currently moving to a new position. False if the focuser is stationary.
        "ismoving" => ApiValue::Bool(focuser_ismoving),
        // maxincrement - Maximum increment size allowed by the focuser; i.e. the maximum number of steps allowed in one move operation.
        "maxincrement" => ApiValue::I32(100),
        // maxstep - Maximum step position permitted.
        "maxstep" => ApiValue::I32(10000),
        // position - Current focuser position, in steps.
        "position" => ApiValue::I32(data.focuser_position),
        // stepsize - The size of one step, in microns.
        "stepsize" => ApiValue::F64(1.0),
        // tempcomp - Gets the state of temperature compensation mode (if available), else always False.
        "tempcomp" => ApiValue::Bool(false),
        // tempcompavailable - True if the focuser supports temperature compensation.
        "tempcompavailable" => ApiValue::Bool(false),
        // temperature - The current temperature of the focuser, in degrees Celsius.
        "temperature" => ApiValue::F64(25.0),
        // common to all devices
        "connected" => ApiValue::Bool(data.focconnected),
        "connecting" => ApiValue::Bool(data.focconnecting),
        "description" => ApiValue::String("Rusty-scope Focuser".to_string()),
        "driverinfo" => ApiValue::String("ESP32 Alpaca Server".to_string()),
        "driverversion" => ApiValue::String(env!("CARGO_PKG_VERSION").to_string()),
        "interfaceversion" => ApiValue::I32(4),
        "name" => ApiValue::String("Rusty-scope".to_string()),
        "supportedactions" => ApiValue::List(vec!(
            ApiValue::String("".to_string())
        )),
        "devicestate" => ApiValue::Array(vec!(
            ("Connected".to_string(), ApiValue::Bool(data.focconnected)),
            ("IsMoving".to_string(), ApiValue::Bool(false)),
            ("Position".to_string(), ApiValue::I32(data.focuser_position)),
            ("TimeStamp".to_string(), ApiValue::String(scope.system.time.get_state().utc_epoch_base.unwrap().to_string())),
            ("Temperature".to_string(), ApiValue::F64(25.0)),
        )),
        
        _ => ApiValue::Null,
    }
}


