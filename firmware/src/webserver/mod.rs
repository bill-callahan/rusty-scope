#![allow(dead_code)]
use std::fs::File;
use std::io::{Read};

use esp_idf_svc::http::server::Configuration;
use esp_idf_svc::http::{server::{EspHttpConnection, EspHttpServer, Request}, Method};
use esp_idf_svc::io::Write;
use log::{error, info};


// The path on the physical filesystem where web files are stored.
const WEB_MOUNT_POINT: &str = "/sdcard"; // Or "/spiffs" if you prefer
const WEB_ROOT_PATH: &str = "/site"; // A sub-directory for web assets

fn file_serving_handler(req: Request<&mut EspHttpConnection>) -> anyhow::Result<()> {
    // The actual web server configuration is done in `config_webserver`.
    // this handler will serve files from the sdcard or internal flash
    /* fileserver Handler Logic:

    It will receive an HTTP Request.

    It will extract the requested path from the request's URI (e.g., /index.html, /style.css).

    It will construct the full path to the file on the SD card (e.g., /sdcard/web/index.html).

    It will attempt to open and read the file.

    If the file is found, it will guess the MIME type based on the file extension.

    It will write the correct HTTP headers (Content-Type, Content-Length).

    It will stream the file's contents to the response.

    If the file is not found, it will send a 404 Not Found response. */
// 1. Get the path from the request URI.
    let uri = req.uri();
    // Default to index.html if the path is just "/"
    let file_path = if uri == "/" {
        "/index.html"
    } else {
        uri
    };
    
    let full_fs_path = format!("{}{}{}", WEB_MOUNT_POINT, WEB_ROOT_PATH, file_path);
    info!("File server received request for URI: '{}', trying path: '{}'", uri, full_fs_path);
    
    // 2. Try to open the file.
    match File::open(&full_fs_path) {
        Ok(mut file) => {
            // --- FIX: Set headers during response creation ---
            let content_type = mime_type_from_path(file_path);
            let headers = [("Content-Type", content_type)];
            let mut response = req.into_response(200, Some("OK"), &headers)?;
            
            // --- The `write_all` calls will now work because the trait is in scope ---
            let mut buffer = [0u8; 1024];
            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 { break; }
                response.write_all(&buffer[..bytes_read])?;
            }
        }
        Err(e) => {
            error!("File not found: {}. Error: {}", full_fs_path, e);
            let mut response = req.into_response(404, Some("Not Found"), &[])?;
            response.write_all(b"404 Not Found")?;
        }
    }
    
    Ok(())
}

/// A simple helper to guess the MIME type from a file extension.
fn mime_type_from_path(path: &str) -> &'static str {
    match path.split('.').last() {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        _ => "text/plain",
    }
}


pub fn create_webserver() -> anyhow::Result<EspHttpServer<'static>> {
    let server_config = Configuration {
        stack_size: 8192, // Increased stack size for file I/O
        http_port: 80,
        ctrl_port: 32768,
        max_open_sockets: 2,
        max_sessions: 2,
        session_timeout: std::time::Duration::from_secs(20),
        uri_match_wildcard: true, // Enable wildcard matching for URIs
        ..Default::default()
    };
    EspHttpServer::new(&server_config).map_err(|e| e.into())
}

pub fn config_webserver(server: &mut EspHttpServer<'static>) -> anyhow::Result<()> {
    // Register the file server handler for any GET request.
    // The `uri_match_wildcard` in the config makes this work.
    server.fn_handler("/", Method::Get, file_serving_handler)?;
    // You could also register a specific handler for `/` and a wildcard for `/*`
    server.fn_handler("/*", Method::Get, file_serving_handler)?;
    
    info!("Web server configured. File server is active on all GET routes.");
    Ok(())
}
