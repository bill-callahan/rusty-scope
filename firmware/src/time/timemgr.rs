// src/time/timemgr.rs

use std::sync::{Arc, Mutex};
use hifitime::Epoch;
//use log::{debug, info};
use log::{info};
use crate::gps::GpsController; // Dependency on the GPS module
//use esp_idf_svc::{hal::delay::FreeRtos, sys::{uxTaskGetStackHighWaterMark, xTaskGetCurrentTaskHandle}};
use esp_idf_svc::{hal::delay::FreeRtos};
use std::thread;

// This struct will hold all time-related state.
#[derive(Debug, Clone)]
pub struct TimeState {
    //pub local_sidereal_time: f64, // In decimal hours
    pub utc_epoch_base: Option<Epoch>,
    pub site_longitude: f64,
}

impl Default for TimeState {
    fn default() -> Self {
        Self {
            utc_epoch_base: None,
            site_longitude: 0.0, // Default to Greenwich
        }
    }
}

// The main manager struct.
pub struct TimeManager {
    state: Mutex<TimeState>,
}

impl TimeManager {
    /// Creates a new TimeManager and spawns its background update task.
    pub fn new(gps_controller: Arc<GpsController>) -> Arc<Self> {
        let manager = Arc::new(Self {
            state: Mutex::new(TimeState::default()),
        });

        let task_manager = Arc::clone(&manager);
        
        thread::Builder::new()
            .name("sidereal_task".into())
            .stack_size(8 * 1024)
            .spawn(move || {
                info!("Time management task started.");
                // let sidereal_task_handle = unsafe { xTaskGetCurrentTaskHandle() };
                let mut loop_counter = 0;
                // Wait until we have a valid GPS position to set the longitude
                loop {
                    let gps_data = gps_controller.get_data();
                    if gps_data.has_fix {
                        info!("GPS fix acquired. Setting site longitude to {}.", gps_data.longitude);
                        let mut state = task_manager.state.lock().unwrap();
                        state.site_longitude = gps_data.longitude;
                        break; // Exit the setup loop
                    }
                    FreeRtos::delay_ms(2000); // Wait for fix
                }

                // Main update loop
                loop {
                    task_manager.update();
                    loop_counter += 1;
                    // Print stats every 300 iterations (e.g., every 30 seconds if loop delay is 100ms)
                    if loop_counter >= 30 {
                        // let stack_hwm = unsafe { uxTaskGetStackHighWaterMark(sidereal_task_handle) };
                        // debug!("[SYS_MONITOR] Sidereal Task - Stack HWM: {} bytes", stack_hwm);
                        loop_counter = 0;
                    }
                    // LST changes significantly over seconds, so update frequently.
                    FreeRtos::delay_ms(1000); // Update LST every second
                }
            })
            .expect("Failed to spawn time_task");

        manager
    }

    /// The internal update function, called by the background task.
    fn update(&self) {
        if let Ok(mut state) = self.state.lock() {
            if let Ok(now) = Epoch::now() {
                state.utc_epoch_base = Some(now);
                // state.local_sidereal_time = Self::compute_lmst(now, state.site_longitude);
            }
        }
    }
    
    /// The core LST calculation. Now a static method.
    fn compute_lmst(now: Epoch, longitude: f64) -> f64 {
        const JD2000: f64 = 2451545.0;
        let jd_ut1 = now.to_jde_tai_days(); // Use TAI for highest accuracy LST

        let d = jd_ut1 - JD2000;
        let gmst_hours = 18.697374558 + 24.06570982441908 * d;
    
        let lst = (gmst_hours + longitude / 15.0) % 24.0;
    
        if lst < 0.0 { lst + 24.0 } else { lst }
    }

    // --- Public API ---
    
    /// Gets a full copy of the current time state.
    pub fn get_state(&self) -> TimeState {
        self.state.lock().unwrap().clone()
    }

   /// Convenience getter for Local Sidereal Time.
    pub fn get_lst(&self) -> f64 {
        // --- 1. Acquire the lock and copy ALL data needed ---
        let (base_epoch_opt, longitude) = {
            let state = self.state.lock().unwrap();
            (state.utc_epoch_base, state.site_longitude)
        }; // <-- Lock is released here.

        // --- 2. Now, work with the local copies ---
        if let Some(_base_epoch) = base_epoch_opt {
            // Get the EXACT current time. This does not require a lock.
            if let Ok(now) = Epoch::now() {
                // Perform the calculation using the local `longitude` variable.
                Self::compute_lmst(now, longitude)
            } else {
                // Epoch::now() can fail if the system time is not yet set.
                0.0
            }
        } else {
            // The sidereal_task hasn't stored the base epoch yet.
            0.0
        }
    }

    /// Convenience getter for UTC Epoch.
    pub fn get_utc_epoch(&self) -> Option<Epoch> {
        self.state.lock().unwrap().utc_epoch_base
    }

    /// Convenience getter for UTC string
    pub fn get_utc_string(&self) -> String {
        if let Some(epoch) = self.get_utc_epoch() {
            epoch.to_gregorian_str(hifitime::TimeScale::UTC)
        } else {
            "N/A".to_string()
            }
    }

    /// Returns the current UTC time as a string formatted specifically for the Alpaca API.
    /// Format: YYYY-MM-DDTHH:MM:SS.sssZ
    pub fn get_utc_string_alpaca(&self) -> String {
        if let Some(epoch) = self.get_utc_epoch() {
            // 1. Get the components of the date and time.
            let (year, month, day, hour, minute, second, nanoseconds) = epoch.to_gregorian(hifitime::TimeScale::UTC);
            
            
            // 2. Convert nanoseconds to milliseconds (3 digits).
            let milliseconds = nanoseconds / 1_000_000;
            
            // 3. Manually format the string to the exact specification.
            let ttt = format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
                year, month, day, hour, minute, second, milliseconds
            );
            ttt
        } else {
            // Return a valid-looking but clearly "not set" time if we don't have one.
            "0001-01-01T00:00:00.000Z".to_string()
        }
    }
}
