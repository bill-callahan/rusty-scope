use esp_idf_svc::http::server::{Request, Response, EspHttpConnection};
use log::info;
use serde::Deserialize;

// Required for reading body
use std::io::Read;


#[derive(Debug, Deserialize)]
pub struct TelescopeForm {
    steps_per_degree: u32,
    tracking_rate: String,
    mount_type: String,
    enable_limits: Option<String>, // will be Some("on") if checkbox checked
    enable_gps: Option<String>,
}

pub fn tsetup_confighandler() -> impl Fn(
    Request<&mut EspHttpConnection<'_>>,
) -> Result<(), ()> + Send + Sync + 'static {
    move |mut req| {
        // --- Check Method ---
        if req.method() != "POST" {
            let mut resp = Response::wrap(req.connection());
            let _ = resp.set_status(405);
            let _ = resp.write_all(b"Method Not Allowed");
            return Ok(());
        }

        // --- Read Body ---
        let mut body = Vec::with_capacity(1024);
        match req.payload().read_to_end(&mut body) {
            Ok(_) => {}
            Err(e) => {
                let mut resp = Response::wrap(req.connection()).unwrap();
                let _ = resp.set_status(400);
                let _ = resp.write_all(format!("Failed to read body: {:?}", e).as_bytes());
                return Ok(());
            }
        };

        let body_str = match core::str::from_utf8(&body) {
            Ok(s) => s,
            Err(_) => {
                let mut resp = Response::wrap(req.connection()).unwrap();
                let _ = resp.set_status(400);
                let _ = resp.write_all(b"Invalid UTF-8 in body");
                return Ok(());
            }
        };

        // --- Parse Form Data ---
        let form_data: TelescopeForm = match serde_urlencoded::from_str(body_str) {
            Ok(data) => data,
            Err(e) => {
                info!("Failed to parse form: {}", e);
                let mut resp = Response::wrap(req.connection()).unwrap();
                let _ = resp.set_status(400);
                let _ = resp.write_all(b"Invalid form data");
                return Ok(());
            }
        };

        // --- Log Parsed Values ---
        info!("Received configuration:");
        info!("Steps per degree: {}", form_data.steps_per_degree);
        info!("Tracking rate: {}", form_data.tracking_rate);
        info!("Mount type: {}", form_data.mount_type);
        info!("Enable limits: {:?}", form_data.enable_limits);
        info!("Use GPS: {:?}", form_data.enable_gps);

        // --- Success Response ---
        let html = "<html><body><h1>Settings Saved!</h1>\
                    <p><a href=\"/telescope/0/configure\">Back to Setup</a></p>\
                    </body></html>";

        let mut resp = Response::wrap(req.connection()).unwrap();
        let _ = resp.set_status(200);
        let _ = resp.set_header("Content-Type", "text/html");
        let _ = resp.write_all(html.as_bytes());

        Ok(())
    }
}