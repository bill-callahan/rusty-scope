
// src/mount/alpacaserver.rs


use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::http::server::{EspHttpConnection, EspHttpServer, Request};
use esp_idf_svc::http::Method;
use esp_idf_svc::sys::{uxTaskGetStackHighWaterMark, xTaskGetCurrentTaskHandle, HttpStatus_Code_HttpStatus_BadRequest, HttpStatus_Code_HttpStatus_Forbidden, HttpStatus_Code_HttpStatus_NotFound, HttpStatus_Code_HttpStatus_Ok};
use esp_idf_svc::sys::HttpStatus_Code as StatusCode;
use log::info;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::Serialize;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use embedded_svc::io::Write;


use crate::mount::alpacadiscovery::create_alpaca_discovery;
use crate::mount::focuser::get::handle_focuser_get;
use crate::mount::focuser::put::handle_focuser_put;
use crate::mount::management::get::handle_management_get;
use crate::mount::management::put::handle_management_put;
use crate::mount::telescope::get::handle_telescope_get;
use crate::mount::telescope::put::handle_telescope_put;
use crate::mount::RustyScope;




static INDEX_HTML: &str = include_str!("management/msetup.html");
static TSETUP_HTML: &str = include_str!("telescope/tsetup.html");
static FSETUP_HTML: &str = include_str!("focuser/fsetup.html");

pub(crate) static SERVERTID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);


// --- This is the entry point for our new Alpaca task ---
pub extern "C" fn alpaca_task_entry(arg: *mut core::ffi::c_void) {
    // 1. Safely reconstitute the Arc from the raw pointer.
    // This is safe because we leaked the original Box in `main`.
    let scope_handle: Arc<RustyScope> = unsafe { Arc::from_raw(arg as *const RustyScope) };
    
    // Now we are in a normal Rust context within this new task.
    info!("Alpaca server task started on core {:?}.", esp_idf_svc::hal::cpu::core());
    let alpaca_task_handle = unsafe { xTaskGetCurrentTaskHandle() };
    let stack_hwm = unsafe { uxTaskGetStackHighWaterMark(alpaca_task_handle) };
            log::info!(
                "[SYS_MONITOR] Alpaca Task - Stack HWM: {} bytes",
                stack_hwm
            );
    let mut loop_counter = 0;
    
    // Get the IP address needed for discovery. We can get it from the scope's system reference.
    let ipaddr = scope_handle.system.ipaddr; // You'll need to add this getter to System

    // Create and configure the server inside the task.
    let mut server = create_alpacaserver().expect("Failed to create Alpaca server");
    config_alpacaserver(&mut server, scope_handle).expect("Failed to configure Alpaca server");
    
    info!("Alpaca server running at http://{}:3000", ipaddr);
    create_alpaca_discovery(ipaddr);
    
    // This task must not exit, or the server will be dropped.
    loop {
        loop_counter += 1;
        // Print stats every 300 iterations (e.g., every 30 seconds if loop delay is 100ms)
        if loop_counter >= 30 {
            let stack_hwm = unsafe { uxTaskGetStackHighWaterMark(alpaca_task_handle) };
            log::info!(
                "[SYS_MONITOR] Alpaca Task - Stack HWM: {} bytes",
                stack_hwm
            );
            loop_counter = 0;
        }
        FreeRtos::delay_ms(100);
    }
}


pub fn config_alpacaserver(server: &mut EspHttpServer<'static>, scope: Arc<RustyScope<'static>>) -> Result<(), anyhow::Error> {
    server.fn_handler("/setup", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(INDEX_HTML.as_bytes())
            .map(|_| ())
    })?;

    server.fn_handler("/setup/v1/telescope/0/setup", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(TSETUP_HTML.as_bytes())
            .map(|_| ())
    })?;

    //server.fn_handler("/setup/v1/telescope/0/setup", Method::Post, tsetup_confighandler())?;
    
    server.fn_handler("/setup/v1/focuser/0/setup", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(FSETUP_HTML.as_bytes())
            .map(|_| ())
    })?;
    //server.fn_handler("/setup/v1/focuser/0/setup", Method::Post, fsetup_confighandler())?;


    // server.fn_handler("/api/v1/*", Method::Get, api_gethandler(Arc::clone(&scope)))?;
    server.fn_handler("/api/v1/*", Method::Get, api_gethandler(Arc::clone(&scope)))?;
    server.fn_handler("/management/*", Method::Get, api_gethandler(Arc::clone(&scope)))?;
    
    server.fn_handler("/api/v1/*", Method::Put, api_puthandler(Arc::clone(&scope)))?;

    // A single, unified PUT handler for all configuration changes.
    // This simplifies the logic. The JavaScript will send to different URLs,
    // but they can all be handled by the same function on the backend.
    server.fn_handler("/management/v1/configure/*", Method::Put, mgmt_puthandler(Arc::clone(&scope)))?;

    // This will handle requests like PUT /management/v1/reboot
    server.fn_handler("/management/v1/*", Method::Put, mgmt_puthandler(Arc::clone(&scope)))?;

    Ok(())
}
pub fn create_alpacaserver() -> Result<EspHttpServer<'static>, anyhow::Error> {
    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: 6144,
        http_port: 3000,
        ctrl_port: 32769,
        max_uri_handlers: 8,
        uri_match_wildcard: true,
        max_open_sockets: 10,
        max_sessions: 10,
        session_timeout: std::time::Duration::from_secs(20),
        ..Default::default()
    };
    EspHttpServer::new(&server_configuration)
        .map_err(|e| e.into())
}



#[derive(Debug, PartialEq)]
pub enum ApiValue {
    F64(f64),
    F32(f32),
    I32(i32),
    Bool(bool),
    String(String),
    List(Vec<ApiValue>),
    KeyPairs(Vec<(String, ApiValue)>),
    Array(Vec<(String, ApiValue)>),
    ArrayI(Vec<i32>),
    Null,
}

impl Default for ApiValue {
    fn default() -> Self {
        ApiValue::Null
    }
}

impl Serialize for ApiValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ApiValue::F64(v) => serializer.serialize_f64(*v),
            ApiValue::F32(v) => serializer.serialize_f32(*v),
            ApiValue::I32(v) => serializer.serialize_i32(*v),
            ApiValue::Bool(v) => serializer.serialize_bool(*v),
            ApiValue::String(v) => serializer.serialize_str(v),
            ApiValue::List(v) => {
                let mut seq = serializer.serialize_seq(Some(v.len()))?;
                for item in v {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            ApiValue::KeyPairs(v) => {
                let mut map = serializer.serialize_map(Some(v.len()))?;
                for (key, value) in v {
                    map.serialize_entry(&key, &value)?;
                }
                map.end()
            }
            ApiValue::Array(v) => {
                let mut seq = serializer.serialize_seq(Some(v.len()))?;
                for (name, value) in v {
                    seq.serialize_element(&serde_json::json!({
                        "Name": name,
                        "Value": value
                    }))?;
                }
                seq.end()
            }
            ApiValue::ArrayI(v) => {
                let mut seq = serializer.serialize_seq(Some(v.len()))?;
                for item in v {
                    seq.serialize_element(&item)?;
                }
                seq.end()
            }
            ApiValue::Null => serializer.serialize_none(),
        }
    }}

#[derive(Serialize)]
pub struct ApiGetResponse {
    #[serde(rename = "ClientTransactionID")]
    client_transaction_id: u32,
    #[serde(rename = "ServerTransactionID")]
    server_transaction_id: u32,
    #[serde(rename = "ErrorNumber")]
    error_number: u32,
    #[serde(rename = "ErrorMessage")]
    error_message: String,
    #[serde(rename = "Value")]
    value: ApiValue,
}

#[derive(Debug, Serialize)]
pub struct ApiPutResponse {
    #[serde(rename = "ClientTransactionID")]
    client_transaction_id: u32,
    #[serde(rename = "ServerTransactionID")]
    server_transaction_id: u32,
    #[serde(rename = "ErrorNumber")]
    error_number: u32,
    #[serde(rename = "ErrorMessage")]
    error_message: String,
}

#[derive(Debug, PartialEq)]
pub enum EndpointType {
    Telescope,
    Focuser,
    Management,
}

// All methods that use the HTTP GET verb should include parameters as query string name-value pairs
// pub fn api_gethandler<'s>(scope: Arc<RustyScope<'static>>) -> impl for<'a> Fn(Request<&'a mut EspHttpConnection>) -> anyhow::Result<()> + Clone + 's {
pub fn api_gethandler(scope: Arc<RustyScope<'static>>) -> impl Fn(Request<&mut EspHttpConnection>) -> anyhow::Result<()> + 'static {

    move |req| {
        //println!("GET-handler Thread: {:?} on core {:?}", std::thread::current().id(), esp_idf_svc::hal::cpu::core());
        //let alpacaget_task_handle = unsafe { xTaskGetCurrentTaskHandle() };
        //let stack_hwm = unsafe { uxTaskGetStackHighWaterMark(alpacaget_task_handle) };
        //    log::info!(
        //        "[SYS_MONITOR] AlpacaGet Task - Stack HWM: {} bytes",
        //        stack_hwm
        //    );
        //println!("Free heap: {} bytes", unsafe { esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_DEFAULT) });
        // Validate HTTP method
        if req.method() != Method::Get {
            return send_error_response(req, HttpStatus_Code_HttpStatus_Forbidden, 0x405, "Method not allowed".to_string(), 0);
        }

        let uri = req.uri();
        let (path, query) = uri.split_once('?').unwrap_or((uri, ""));

        // Parse endpoint
        let (endpoint_type, endpoint) = match path.strip_prefix("/api/v1/telescope/0/") {
            Some(e) => (EndpointType::Telescope, e),
            None => match path.strip_prefix("/api/v1/focuser/0/") {
                Some(e) => (EndpointType::Focuser, e),
                None => match path.strip_prefix("/management/") {
                    Some(e) => (EndpointType::Management, e),
                    None => {
                        if path == "/api/v1/telescope/0" || path == "/api/v1/focuser/0" || path == "/management" {
                            (EndpointType::Management, "")
                        } else {
                            return send_error_response(req, HttpStatus_Code_HttpStatus_NotFound, 0x404, "Invalid endpoint".to_string(), 0);
                        }
                    }
                }
            }
        };

        
        //info!("Received {:?} GET request for endpoint: {}", endpoint_type, endpoint);
        //info!("Query: {:?}", query);

        // Parse query parameters
        let mut client_id = 0;
        let mut client_transaction_id = 0;
        let mut axis: Option<i32> = None;

        if endpoint_type != EndpointType::Management {
            // Parse query parameters
            for param in query.split('&') {
                if let Some((key, value)) = param.split_once('=') {
                    match key.to_lowercase().as_str() {
                        "clientid" => client_id = value.parse().unwrap_or(-99),
                        "clienttransactionid" => client_transaction_id = value.parse().unwrap_or(0),
                        "axis" => axis = value.parse().ok(),
                        _ => {}
                    }
                }
            }
        
        

            // Validate ClientID only for device endpoints
            if client_id < 0 || client_transaction_id == 0 {
                return send_error_response(
                    req,
                    HttpStatus_Code_HttpStatus_BadRequest,
                    0x400,
                    "Invalid ClientID or transaction id".to_string(),
                    client_transaction_id,
                );
            }
        }

        // Handle endpoint
        let value = match endpoint_type {
            EndpointType::Telescope => handle_telescope_get(Arc::clone(&scope), endpoint, axis),
            EndpointType::Focuser => handle_focuser_get(Arc::clone(&scope), endpoint),
            EndpointType::Management => handle_management_get(Arc::clone(&scope), endpoint),
        };

        // Build response
        let response = if value == ApiValue::Null {
            ApiGetResponse {
                client_transaction_id,
                server_transaction_id: SERVERTID.fetch_add(1, Ordering::Relaxed),
                error_number: 0x400,
                error_message: "Property or method not implemented".to_string(),
                value: ApiValue::Null,
            }
        } else {
            ApiGetResponse {
                client_transaction_id,
                server_transaction_id: SERVERTID.fetch_add(1, Ordering::Relaxed),
                error_number: 0,
                error_message: "".to_string(),
                value,
            }
        };
        //info!("Response: {}", serde_json::to_string(&response).unwrap());
        // Send response
        if response.error_number != 0 {
            send_error_response(req, HttpStatus_Code_HttpStatus_BadRequest, response.error_number, response.error_message, response.client_transaction_id)
        } else {
            send_json_gresponse(req, HttpStatus_Code_HttpStatus_Ok, response)
        }
        //send_json_gresponse(req, HttpStatus_Code_HttpStatus_Ok, response)
    }

}


fn send_json_gresponse(
    req: Request<&mut EspHttpConnection>,
    status: StatusCode,
    response: ApiGetResponse,
) -> anyhow::Result<()> {
    let body = serde_json::to_string(&response)?;
    let mut res = req.into_response(status as u16, None, &[
        ("Content-Type", "application/json"),
        ("Access-Control-Allow-Origin", "*"),
        ("Server", "Rusty-scope Alpaca Server"),
    ])?;
    res.write_all(body.as_bytes())?;
    Ok(())
}

fn send_json_presponse(
    req: Request<&mut EspHttpConnection>,
    status: StatusCode,
    response: ApiPutResponse,
) -> anyhow::Result<()> {
    let body = serde_json::to_string(&response)?;
    let mut res = req.into_response(status as u16, None, &[
        ("Content-Type", "application/json"),
        ("Access-Control-Allow-Origin", "*"),
        ("Server", "Rusty-scope Alpaca Server"),
    ])?;
    res.write_all(body.as_bytes())?;
    Ok(())
}

fn send_error_response(
    req: Request<&mut EspHttpConnection>,
    status: StatusCode,
    error_number: u32,
    message: String,
    client_transaction_id: u32,
) -> anyhow::Result<()> {
    let response = ApiGetResponse {
        client_transaction_id,
        server_transaction_id: SERVERTID.fetch_add(1, Ordering::Relaxed),
        error_number,
        error_message: message,
        value: ApiValue::Null,
    };
    send_json_gresponse(req, status, response)
}


// All methods that use the HTTP PUT verb should include name-value parameters in the body using the "application/x-www-form-urlencoded" media type. 
pub fn api_puthandler(scope: Arc<RustyScope<'static>>) -> impl for<'a> Fn(Request<&'a mut EspHttpConnection>) -> anyhow::Result<()> + Clone + 'static {
    move |mut req| {
        //println!("PUT-handler Thread: {:?} on core {:?}", std::thread::current().id(), esp_idf_svc::hal::cpu::core());
        //let alpacaput_task_handle = unsafe { xTaskGetCurrentTaskHandle() };
        //let stack_hwm = unsafe { uxTaskGetStackHighWaterMark(alpacaput_task_handle) };
        //    log::info!(
        //        "[SYS_MONITOR] AlpacaPut Task - Stack HWM: {} bytes",
        //        stack_hwm
        //    );
        //println!("Free heap: {} bytes", unsafe { esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_DEFAULT) });
        // Validate HTTP method
        if req.method() != Method::Put {
            return send_error_response(
                req, 
                0x405, 
                0x405, 
                "Method not allowed".to_string(), 
                0
            );
        }

        let uri = req.uri().to_string();
        let (path, _) = uri.split_once('?').unwrap_or((&uri, ""));

        // Parse endpoint
        let (endpoint_type, endpoint) = match path.strip_prefix("/api/v1/telescope/0/") {
            Some(e) => (EndpointType::Telescope, e),
            None => match path.strip_prefix("/api/v1/focuser/0/") {
                Some(e) => (EndpointType::Focuser, e),
                None => {
                    if path == "/api/v1/telescope/0" || path == "/api/v1/focuser/0" {
                        (EndpointType::Management, "")
                    } else {
                        return send_error_response(
                            req, 
                            HttpStatus_Code_HttpStatus_NotFound, 
                            0x404, 
                            "Invalid endpoint".to_string(), 
                            0
                        );
                    }
                }
            }
        };

        info!("Received {:?} PUT request for endpoint: {}", endpoint_type, endpoint);
        

        // Read the PUT body data
        let mut buf = [0u8; 1024];
        let len = req.read(&mut buf)?;
        let body = std::str::from_utf8(&buf[..len])?;
        // Parse form-urlencoded data
        let mut params = std::collections::HashMap::new();
        for param in body.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                params.insert(key.to_string(), value.to_string());
            }
        }

        info!("Received parameters: {}", params.keys().map(|k| format!("{}: {}", k, params[k])).collect::<Vec<_>>().join(", "));

        // Extract common parameters
        let clientid: u32 = params.get("ClientID")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
        let client_transaction_id = params.get("ClientTransactionID")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Handle the PUT request based on endpoint
        let result = match endpoint_type {
            EndpointType::Telescope => handle_telescope_put(Arc::clone(&scope), endpoint, &params),
            EndpointType::Focuser => handle_focuser_put(Arc::clone(&scope), endpoint, &params),
            EndpointType::Management => Err("Invalid root endpoint for PUT".to_string()),
        };

        // Build response
        let (error_number, error_message) = match result {
            Ok(_) => (0, String::new()),
            Err(e) => (0x400, e),
        };

        let response = ApiPutResponse {
            client_transaction_id,
            server_transaction_id: SERVERTID.fetch_add(1, Ordering::Relaxed),
            error_number,
            error_message,
        };

        //info!("Sending PUT response: {:?}", response);

        if clientid == 0 || client_transaction_id == 0 {
            return send_error_response(
                req,
                HttpStatus_Code_HttpStatus_BadRequest,
                0x400,
                "ClientID or ClientTransactionID is missing".to_string(),
                client_transaction_id
            );
        }

        send_json_presponse(req, HttpStatus_Code_HttpStatus_Ok, response)
    }
}

pub fn mgmt_puthandler(scope: Arc<RustyScope<'static>>) -> impl Fn(Request<&mut EspHttpConnection>) -> anyhow::Result<()> + Send + 'static {
    move |mut req| {
        let uri = req.uri().to_string();
        // Strip the prefix to get the endpoint (e.g., "reboot")
        let endpoint = uri.strip_prefix("/management/").unwrap_or(&uri);

        info!("[API] Management PUT request for endpoint: '{}'", endpoint); // Add a log for debugging!
        
        // Read the body of the request
        let mut buf = [0u8; 1024];
        let len = req.read(&mut buf)?;
        let body = std::str::from_utf8(&buf[..len])?;

        // Call your logic handler
        let result = handle_management_put(Arc::clone(&scope), endpoint, Some(body));

        // Build and send a standard success/error response
        let (error_number, error_message) = match result {
            Ok(_) => (0, String::new()),
            Err(e) => (0x400, e), // Use a generic "Bad Request" error number
        };
        
        // This is a simple response, not a full Alpaca one, but that's okay for UI.
        let response_json = serde_json::json!({
            "ErrorNumber": error_number,
            "ErrorMessage": error_message,
        });

        let mut res = req.into_ok_response()?;
        res.write_all(serde_json::to_string(&response_json)?.as_bytes())?;
        
        Ok(())
    }
}