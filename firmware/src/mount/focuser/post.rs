
use esp_idf_svc::{
    http::server::{Request, EspHttpConnection},
    io::{EspIOError, Write}

};
use anyhow::Result;
use log;


pub fn fsetup_confighandler(mut req: Request<EspHttpConnection>) -> Result<()> {
    let mut buf = [0u8; 256];
    let bytes_read = req.read(&mut buf)?;
    let body = String::from_utf8_lossy(&buf[..bytes_read]);

    log::info!("Received POST data: {}", body);

    let mut response = req.into_response(200, None, &[
        ("Content-Type", "text/plain"),
        ("Connection", "close")
    ])?;

    response.write_all(b"Configuration received")?;

    Ok(())
}

