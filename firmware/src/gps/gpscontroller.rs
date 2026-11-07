// src/gps/gpscontroller.rs
use esp_idf_svc::{
    hal::{delay::FreeRtos, uart::UartDriver},
    // sys::{settimeofday, timeval, uxTaskGetStackHighWaterMark, xTaskGetCurrentTaskHandle, EspError}
    sys::{settimeofday, timeval, EspError}
    ,
};
use hifitime::{Epoch, TimeScale};
use log::{error, info, debug, warn};

use std::{
    sync::{
        Arc, Mutex,
    }, thread,
};

// This struct will hold all the data parsed from the GPS.
#[derive(Debug, Clone, Default)]
pub struct GpsData {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: f64,
    pub has_fix: bool,
    pub speed_kph: Option<f64>, // Speed in km/h, optional if not available
    pub epoch_utc: Option<Epoch>, // Optional UTC epoch time
    pub satellites_used: u8, // Number of satellites used for the fix
    // You can add more fields as needed, such as:
    // pub course_over_ground: Option<f64>, // Course over ground in degrees
    // pub utc_time: Option<Epoch>, // UTC time of the fix
    // pub date: Option<String>, // Date of the fix in "ddmmyy" format
    // pub hdop: Option<f64>, // Horizontal Dilution of Precision
    // pub vdop: Option<f64>, // Vertical Dilution of Precision
    // pub pdop: Option<f64>, // Position Dilution of Precision
    // pub age_of_differential_gps_data: Option<f64>, // Age of differential GPS data in seconds
    // pub dgps_station_id: Option<String>, // ID of the differential GPS station
    // pub magnetic_variation: Option<f64>, // Magnetic variation in degrees
    // pub magnetic_variation_direction: Option<String>, // Direction of magnetic variation (E
    // You can add utc_time, date, etc. here as well
}

// The main controller struct. It holds the hardware and the shared data.
pub struct GpsController {
    // We don't need to store the UartDriver here if it's moved to the task.
    // Instead, we store the data that the task will update.
    data: Mutex<GpsData>,
}

impl GpsController {
    // The `new` function now sets up the hardware and spawns the background task.
    pub fn new(
        uart_driver: UartDriver<'static>,
    ) -> Result<Arc<Self>, EspError> {
        
        let controller = Arc::new(Self {
            data: Mutex::new(GpsData::default()),
        });

        let task_controller = Arc::clone(&controller);
        
        let _ = thread::Builder::new()
            .name("gps_task".into())
            .stack_size(8192) // Set a reasonable stack size
            .spawn(move || {
                //let gps_task_handle = unsafe { xTaskGetCurrentTaskHandle() };
                let mut loop_counter = 0;
                // The `uart_driver` is moved into and now owned by this thread.
                let uart = uart_driver;

                // --- Configure the GPS module at startup ---
                if let Err(e) = Self::configure_module(&uart) {
                    error!("Failed to configure GPS module: {}", e);
                    // We can continue anyway, it might work with defaults.
                }
                let mut read_buf = [0u8; 256];
                // let mut partial_sentence = String::new();
                let mut frame_buf: Vec<u8> = Vec::with_capacity(512);

                let mut current_data = GpsData::default(); // Hold the latest data here

                info!("GPS parsing task started.");
                loop {
                    // Read raw bytes from the UART.
                    if let Ok(len) = uart.read(&mut read_buf, 200) {
                        if len > 0 {
                            frame_buf.extend_from_slice(&read_buf[..len]);
                            
                            // Process all complete frames in the buffer.
                            loop {
                                // 1. Find the start of a frame ('$').
                                if let Some(start_idx) = frame_buf.iter().position(|&b| b == b'$') {
                                    // 2. Find the end of the frame ('\n') after the start.
                                    if let Some(end_idx) = frame_buf[start_idx..].iter().position(|&b| b == b'\n') {
                                        // The full end index is relative to the start.
                                        let final_end_idx = start_idx + end_idx;

                                        // 3. We have a complete frame.
                                        let sentence_bytes = &frame_buf[start_idx..final_end_idx];

                                        // 4. Attempt to convert this validated frame to a UTF-8 string.
                                        // Since NMEA is pure ASCII, this will only succeed if there are
                                        // no corrupted, non-ASCII bytes in the frame.
                                        if let Ok(sentence_str) = str::from_utf8(sentence_bytes) {
                                            //info!("GPS Sentence: {}", sentence_str.trim());
                                            // 5. If successful, parse the now-guaranteed-valid string.
                                            parse_nmea_sentence(sentence_str.trim(), &mut current_data);
                                        }
                                        // If from_utf8 fails, we silently discard the corrupt frame.

                                        // 6. Remove the processed frame (including the '\n') from the buffer.
                                        frame_buf.drain(..=final_end_idx);

                                        // Continue the inner loop to check for more sentences.
                                        continue;
                                    }
                                }
                                // If we can't find a complete frame, break the inner loop and wait for more data.
                                break;
                            }
                        }
                    }
                    
                    // After parsing a batch of sentences, update the shared state.
                    if let Ok(mut data_guard) = task_controller.data.lock() {
                          *data_guard = current_data.clone();
                    }

                    // To prevent the frame_buf from growing indefinitely with garbage data,
                    // we can add a sanity check.
                    if frame_buf.len() > 1024 {
                        warn!("GPS frame buffer exceeded 1KB, clearing to prevent memory leak from garbage data.");
                        frame_buf.clear();
                    }

                    loop_counter += 1;
                    // Print stats every 300 iterations (e.g., every 30 seconds if loop delay is 100ms)
                    if loop_counter >= 150 {
                        //let stack_hwm = unsafe { uxTaskGetStackHighWaterMark(gps_task_handle) };
                        let factor = crate::gps::pps_sync::get_clock_correction_factor();
                        debug!("[PPS_MONITOR] Current Correction Factor: {}", factor);
                        //debug!("[SYS_MONITOR] GPS Task - Stack HWM: {} bytes", stack_hwm);
                        loop_counter = 0;
                    }
                    FreeRtos::delay_ms(10);
                }
            }); // The '?' will propagate any errors from spawning the thread.

        Ok(controller)
    }

    // --- Public API Methods ---
    pub fn get_data(&self) -> GpsData {
        self.data.lock().unwrap().clone()
    }
    
    pub fn has_fix(&self) -> bool {
        self.data.lock().unwrap().has_fix
    }

    /// Sets the ESP32's system time using the latest available UTC time from the GPS.
    ///
    /// This function will only set the time if a valid UTC epoch is available from a
    /// parsed NMEA sentence (like $GPRMC or $GPZDA).
    ///
    /// # Returns
    /// - `Ok(true)` if the time was successfully set.
    /// - `Ok(false)` if no valid time is available from the GPS yet.
    /// - `Err(EspError)` if the underlying system call fails.
    pub fn set_system_time(&self) -> Result<bool, EspError> {
        let data = self.get_data(); // Use the existing safe getter method

        if let Some(epoch) = data.epoch_utc {
            // Convert the hifitime Epoch to the C-style timeval struct.
            let timev = timeval {
                tv_sec: epoch.to_unix_seconds() as i64, // Seconds since Unix epoch
                tv_usec: 0, // Microseconds part
            };

            info!("Attempting to set system time to: {}", epoch);
            
            // The `settimeofday` function is unsafe because it's a C FFI call.
            let result = unsafe { settimeofday(&timev, std::ptr::null()) };

            if result == 0 {
                // Success
                info!("System time set successfully!");
                Ok(true)
            } else {
                // Failure, return an EspError
                error!("Failed to set system time. Error code: {}", result);
                Err(EspError::from(result).expect("Failed to convert error code to EspError"))
            }
        } else {
            // No time data available from GPS yet
            warn!("Cannot set system time: No UTC epoch available from GPS.");
            Ok(false)
        }
    }

    /// Sends configuration commands to the GPS module.
    /// This should be called once at the start of the `gps_task`.
    fn configure_module(uart: &UartDriver) -> Result<(), EspError> {
        info!("Configuring u-blox GPS module to send only essential NMEA sentences...");

        // Helper function to build a UBX CFG-MSG command for setting an NMEA sentence's rate.
        fn build_set_nmea_rate_cmd(nmea_id: u8, rate: u8) -> [u8; 16] {
            let command_data: [u8; 12] = [
                0x06, 0x01, // Class, ID (CFG-MSG)
                0x08, 0x00, // Length (8 bytes)
                0xF0, nmea_id, // Payload: NMEA Class, Specified NMEA ID
                0x00,       // Rate on I2C
                rate,       // Rate on UART1 (0=disable, 1=enable once per fix)
                0x00,       // Rate on UART2
                0x00,       // Rate on USB
                0x00,       // Rate on SPI
                0x00,       // Rate on Reserved
            ];
            let (ck_a, ck_b) = calculate_ubx_checksum(&command_data);
            let mut packet = [0u8; 16];
            packet[0] = 0xB5;
            packet[1] = 0x62;
            packet[2..14].copy_from_slice(&command_data);
            packet[14] = ck_a;
            packet[15] = ck_b;
            packet
        }
        /// Calculates the 2-byte Fletcher-8 checksum for a u-blox UBX command.
        /// The `data` slice should contain: Class, ID, Length, and Payload.
        fn calculate_ubx_checksum(data: &[u8]) -> (u8, u8) {
            let mut ck_a: u8 = 0;
            let mut ck_b: u8 = 0;

            for byte in data {
                ck_a = ck_a.wrapping_add(*byte);
                ck_b = ck_b.wrapping_add(ck_a);
            }
            (ck_a, ck_b)
        }

        // --- NMEA Message IDs for u-blox modules ---
        const NMEA_ID_GGA: u8 = 0x00;
        const NMEA_ID_GLL: u8 = 0x01;
        const NMEA_ID_GSA: u8 = 0x02;
        const NMEA_ID_GSV: u8 = 0x03;
        const NMEA_ID_RMC: u8 = 0x04;
        const NMEA_ID_VTG: u8 = 0x05;
        const NMEA_ID_ZDA: u8 = 0x08;
        
        const RATE_DISABLE: u8 = 0x00;
        const RATE_ENABLE: u8 = 0x01;

        // --- Sequence of commands ---
        let commands = [
        // --- First, disable the sentences we DON'T want ---
        (build_set_nmea_rate_cmd(NMEA_ID_GLL, RATE_DISABLE), "GLL", RATE_DISABLE),
        (build_set_nmea_rate_cmd(NMEA_ID_GSA, RATE_DISABLE), "GSA", RATE_DISABLE),
        (build_set_nmea_rate_cmd(NMEA_ID_GSV, RATE_DISABLE), "GSV", RATE_DISABLE),
        (build_set_nmea_rate_cmd(NMEA_ID_VTG, RATE_DISABLE), "VTG", RATE_DISABLE),
        
        // --- Second, explicitly enable the sentences we DO want ---
        (build_set_nmea_rate_cmd(NMEA_ID_GGA, RATE_ENABLE), "GGA", RATE_ENABLE),
        (build_set_nmea_rate_cmd(NMEA_ID_RMC, RATE_ENABLE), "RMC", RATE_ENABLE),
        (build_set_nmea_rate_cmd(NMEA_ID_ZDA, RATE_ENABLE), "ZDA", RATE_ENABLE),
        ];

        // --- Send all commands sequentially ---
        for (cmd_bytes, name, rate) in commands {
            uart.write(&cmd_bytes)?;

            // Now we check the `rate` variable directly
            if rate == RATE_ENABLE {
                info!("Sent command to ENABLE ${} sentences.", name);
           } else {
                info!("Sent command to DISABLE ${} sentences.", name);
            }
        
            FreeRtos::delay_ms(100);
        }

        info!("GPS module configuration complete.");
        Ok(())
    }
}




/// Parses a single NMEA sentence and updates the provided GpsData struct.
/// This function is designed to be panic-free and robust against malformed data.
fn parse_nmea_sentence(sentence: &str, data: &mut GpsData) {
    if !sentence.starts_with('$') {
        return;
    }
    let parts: Vec<&str> = sentence.split(',').collect();
    // Use .get() for safe access to the sentence type
    // let sentence_type = match parts.get(0) {
    let sentence_type = match parts.first() {
        Some(s) => s,
        None => return, // Ignore empty lines
    };

    // Format: The NEO-7M outputs standard NMEA 0183 sentences.
    // Talker ID: You will likely see the sentences start with $GNGGA, $GNRMC, etc., instead of $GPGGA. 
    // The GN talker ID is the modern standard, indicating that the fix was derived from a combination of 
    // constellations (e.g., GPS, GLONASS, Galileo). The u-blox 7 series supports GPS and GLONASS concurrently.

    // Slice the sentence to get just the message type, ignoring the talker ID.
    // e.g., "$GPGGA,..." -> "GGA"
    // e.g., "$GNRMC,..." -> "RMC"
    let message_type = sentence_type.get(3..).unwrap_or("");

    // match *sentence_type {
    match message_type {
        "GGA" => {
            // Use a fallible closure to safely parse everything.
            let _ = (|| -> Option<()> {
                let fix_quality = parts.get(6)?.parse::<u8>().ok()?;
                data.has_fix = fix_quality > 0;
                if data.has_fix {
                    data.latitude = parse_coordinate(Some(parts.get(2)?), Some(parts.get(3)?))?;
                    data.longitude = parse_coordinate(Some(parts.get(4)?), Some(parts.get(5)?))?;
                    data.elevation = parts.get(9)?.parse().ok()?;
                    data.satellites_used = parts.get(7)?.parse().ok()?;
                }
                Some(())
            })();
        }
        
        "RMC" => {
            let _ = (|| -> Option<()> {
                let status = parts.get(2)?;
                if *status == "A" {
                    data.has_fix = true;
                    data.latitude = parse_coordinate(Some(parts.get(3)?), Some(parts.get(4)?))?;
                    data.longitude = parse_coordinate(Some(parts.get(5)?), Some(parts.get(6)?))?;
                    let speed_knots = parts.get(7)?.parse::<f64>().ok()?;
                    data.speed_kph = Some(speed_knots * 1.852);
                } else {
                    data.has_fix = false;
                }
                Some(())
            })();
        }

        "GLL" => {
            let _ = (|| -> Option<()> {
                let status = parts.get(6)?;
                if *status == "A" {
                    data.has_fix = true;
                    data.latitude = parse_coordinate(Some(parts.get(1)?), Some(parts.get(2)?))?;
                    data.longitude = parse_coordinate(Some(parts.get(3)?), Some(parts.get(4)?))?;
                } else {
                    data.has_fix = false;
                }
                Some(())
            })();
        }

        "ZDA" => {
            let result: Option<Epoch> = (|| {
                let time_str = parts.get(1)?;
                
                let h = time_str.get(0..2)?.parse::<u8>().ok()?;
                let m = time_str.get(2..4)?.parse::<u8>().ok()?;
                let s = time_str.get(4..6)?.parse::<u8>().ok()?;
                let d = parts.get(2)?.parse::<u8>().ok()?;
                let mon = parts.get(3)?.parse::<u8>().ok()?;
                let y = parts.get(4)?.parse::<i32>().ok()?;

                let nanos = time_str.split('.').nth(1).unwrap_or("000")
                                  .get(0..3).unwrap_or("000")
                                  .parse::<u32>().unwrap_or(0) * 1_000_000;
                
                Epoch::maybe_from_gregorian(y, mon, d, h, m, s, nanos, TimeScale::UTC).ok()
            })();

            if let Some(epoch) = result {
                data.epoch_utc = Some(epoch);
            }
        }
        
        "VTG" => {
            let _ = (|| -> Option<()> {
                let speed_kmh = parts.get(7)?.parse::<f64>().ok()?;
                data.speed_kph = Some(speed_kmh);
                Some(())
            })();
        }
        
        _ => (), 
    }
}

// Ensure the helper is also robust against None from .get()
fn parse_coordinate(value_opt: Option<&str>, direction_opt: Option<&str>) -> Option<f64> {
    let value_str = value_opt?;
    let direction_str = direction_opt?;

    let value = value_str.to_string();
    let direction = direction_str.to_string();
    if value.is_empty() || direction.is_empty() { return None; }
    
    let minutes_start_index = match direction.as_str() {
        "E" | "W" => 3,
        "N" | "S" => 2,
        _ => return None,
    };

    if value.len() <= minutes_start_index { return None; }

    let (deg_str, min_str) = value.split_at(minutes_start_index);
    
    let degrees = deg_str.parse::<f64>().ok()?;
    let minutes = min_str.parse::<f64>().ok()?;
    
    let decimal_degrees = degrees + minutes / 60.0;
    
    match direction.as_str() {
        "N" | "E" => Some(decimal_degrees),
        "S" | "W" => Some(-decimal_degrees),
        _ => None,
    }
}
