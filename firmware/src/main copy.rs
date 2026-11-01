// src/main.rs

// 1. Declare the top-level modules of your application
#[cfg(feature = "display")]
mod ts35;

mod gps;
mod stepper;
mod storage;
mod time;
mod webserver;
mod wifi;
mod systemsvcs;


mod mount;

// use std::ffi::CString;
use std::fs::{read_dir, File};
use std::io::Read;
use std::sync::{Arc, Mutex};

//use anyhow::Ok;
//use anyhow::anyhow;
//use embedded_graphics::pixelcolor::Rgb565;
//use embedded_graphics::prelude::{Point, RgbColor, WebColors};
use esp_idf_svc::hal::delay::FreeRtos;
//use esp_idf_svc::hal::task::CriticalSection;
//use esp_idf_svc::log::EspLogger;
//use esp_idf_svc::sys::{esp_log_level_set, httpd_get_client_list, uxTaskGetStackHighWaterMark, uxTaskPriorityGet, xTaskCreatePinnedToCore, xTaskGetCurrentTaskHandle};
use esp_idf_svc::sys::{uxTaskGetStackHighWaterMark, xTaskCreatePinnedToCore, xTaskGetCurrentTaskHandle};
//use profont::PROFONT_14_POINT;
//use std::fmt::Write as FmtWrite;
//use heapless::String as HeaplessString;

#[cfg(feature = "display")]
use {
    embedded_graphics::pixelcolor::Rgb565,
    embedded_graphics::prelude::{Point, RgbColor, WebColors},
    profont::PROFONT_14_POINT,
    std::fmt::Write as FmtWrite,
    heapless::String as HeaplessString,
    crate::ts35::DisplayState,
};

extern crate pretty_env_logger;


use log::{debug, error, info, set_max_level, trace, warn, LevelFilter};

use crate::mount::alpacadiscovery::create_alpaca_discovery;
use crate::mount::alpacaserver::{alpaca_task_entry, config_alpacaserver, create_alpacaserver};
use crate::mount::{ commander_task_entry, AxisDrivetrain, Declination, RightAscension, RustyScope, RustyScopeState, SharedAppState };
use crate::storage::DeviceConfig;
use crate::systemsvcs::SystemServices;







fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();


    log::info!("--- Telescope Controller Booting Up ---");
    

    // Initialize all hardware and services in one go.
    let system = Box::leak(Box::new(SystemServices::new()?));

    // --- Create the Application State ---
    // This is the single, thread-safe container for all our read/write data.
    let app_state  = Arc::new(Mutex::new(RustyScopeState::default()));

    // --- Create the Controller ---
    // Pass both the system services and the application state to the controller.
    let rustyscope: Arc<RustyScope<'static>> = Arc::new(RustyScope::new(system, Arc::clone(&app_state))?);

    // Alpacaserver thread
    let alpaca_scope_ptr = Arc::into_raw(Arc::clone(&rustyscope));
    // 2. The task name must be a C-style string.
    let alpaca_task_name = std::ffi::CString::new("alpaca_task").unwrap();
    
    // 3. Create the FreeRTOS task.
    unsafe {
        xTaskCreatePinnedToCore(
            Some(alpaca_task_entry),
            alpaca_task_name.as_ptr(),
            12 * 1024, // 12KB stack
            alpaca_scope_ptr as *mut core::ffi::c_void, // Pass the pointer as the argument
            5, // A medium priority for a web server
            core::ptr::null_mut(), // No task handle needed
            1, // Pin to core 1 to leave core 0 free for real-time tasks like the timer ISR
        );
    }

    // Commander thread
    let commander_scope_ptr = Arc::into_raw(Arc::clone(&rustyscope));
    // 2. The task name must be a C-style string.
    let commander_task_name = std::ffi::CString::new("commander_task").unwrap();
    // 3. Create the FreeRTOS task.
    unsafe {
        xTaskCreatePinnedToCore(
            Some(commander_task_entry),
            commander_task_name.as_ptr(),
            12 * 1024, // 12KB stack
            commander_scope_ptr as *mut core::ffi::c_void, // Pass the pointer as the argument
            5, // A medium priority for the commander task
            core::ptr::null_mut(), // No task handle needed
            1, // Pin to core 1 to leave core 0 free for real-time tasks like the timer ISR
        );
    }


    
    // test the sdcard
    {
        let mut file = File::open("/sdcard/test.txt")?;

        info!("File {file:?} opened");

        let mut file_content = String::new();

        file.read_to_string(&mut file_content).expect("Read failed");

        info!("File {file:?} read: {file_content}");

        //assert_eq!(file_content.as_bytes(), content);
    }


    {
        let directory = read_dir("/sdcard")?;

        for entry in directory {
            log::info!("Entry: {:?}", entry?.file_name());
        }
    }

    println!("System running. Happy Astro-ing!!");
    

#[cfg(feature = "display")]
{
    // --- The Main Application Loop: The Dashboard ---
    println!("Starting main dashboard loop...");
    let mut display_controller = system.display.lock().unwrap();
    
    // Perform an initial clear screen ONCE after initialization.
    display_controller.clear_screen()?;
    
    // Create a variable to hold the state of the *previous* frame.
    let mut previous_state = DisplayState::default();
    
    // A reusable buffer for formatting the *current* frame's strings.
    let mut text_buffer = HeaplessString::<64>::new();

    // --- The infinite loop to keep the system alive ---
    
    let mut loop_counter = 0;
    let main_task_handle = unsafe { xTaskGetCurrentTaskHandle() }; // Get handle to this task
    loop {
        // --- 1. Data Fetching Phase ---
        
        // Get a snapshot of the live state from all controllers.
        //let motor_pos_steps = system.planner.lock().unwrap().current_position_steps();
        //let is_slewing = system.planner.lock().unwrap().is_busy();
        let (motor_pos_steps, is_slewing) = {
            let planner_lock = system.planner.lock().unwrap();
            (
                planner_lock.current_position_steps(),
                planner_lock.is_slewing(),
            )
        };  // release the planner lock!!
        let gps_data = system.gps.get_data();
        let time_state = system.time.get_state();
        //let data = rustyscope.app_state.lock().unwrap();

        // --- 2. State Generation Phase ---
        // Create a new `DisplayState` struct representing what *should* be on screen now.
        
        let mut current_state = DisplayState::default();

        // Format header string
        text_buffer.clear();
        write!(text_buffer, "               RustyScope Dashboard").unwrap();
        current_state.header_string = text_buffer.clone();

        // Format AxisX string
        text_buffer.clear();
        write!(text_buffer, "AxisX:  {:.4} steps ", motor_pos_steps[0]).unwrap();
        //write!(text_buffer, "   RA:    {}", data.current_ra.as_hms_string()).unwrap();
        current_state.axis_x_string = text_buffer.clone();

        // Format AxisY string
        text_buffer.clear();
        write!(text_buffer, "AxisY:  {:.4} steps ", motor_pos_steps[1]).unwrap();
        //write!(text_buffer, "  Dec:  {}", data.current_dec.as_dms_string()).unwrap();
        current_state.axis_y_string = text_buffer.clone();

        // Format AxisZ string
        text_buffer.clear();
        write!(text_buffer, "AxisZ:  {:.4} steps", motor_pos_steps[2]).unwrap();
        current_state.axis_z_string = text_buffer.clone();

        // Format Status string
        text_buffer.clear();
        write!(text_buffer, "Status: {}", if is_slewing { "SLEWING" } else { "IDLE   " }).unwrap();
        current_state.status_string = text_buffer.clone();

        // Format UTC string
        text_buffer.clear();
        let ts = gps_data.epoch_utc.unwrap();
        write!(text_buffer, "UTC:   {:?}", ts).unwrap();
        current_state.utc_string = text_buffer.clone();
        
        // Format LST string
        text_buffer.clear();
        //write!(text_buffer, "LST: {:.4} hrs", time_state.local_sidereal_time).unwrap();
        current_state.lst_string = text_buffer.clone();
        
        // Format GPS string
        text_buffer.clear();
        if gps_data.has_fix {
            write!(text_buffer, "GPS: FIX OK ({})", gps_data.satellites_used).unwrap();
        } else {
            write!(text_buffer, "GPS: No Fix").unwrap();
        }
        current_state.gps_string = text_buffer.clone();

        // Format Longitude string
        text_buffer.clear();
        write!(text_buffer, "Longitude: {:.4}°", gps_data.longitude).unwrap();
        current_state.lon_string = text_buffer.clone();

        // Format Latitude string
        text_buffer.clear();
        write!(text_buffer, "Latitude: {:.4}°", gps_data.latitude).unwrap();
        current_state.lat_string = text_buffer.clone();

        // Webserver string
        text_buffer.clear();
        write!(text_buffer, "Webserver: http://{}/", system.ipaddr).unwrap();
        current_state.webserver_string = text_buffer.clone();

        // Built-by string
        text_buffer.clear();
        write!(text_buffer, "https://bill-callahan.github.io/rusty-scope/").unwrap();
        current_state.builtby_string = text_buffer.clone();

        // Version string
        text_buffer.clear();
        write!(text_buffer, "Firmware: {} {} by {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), "Bill Callahan").unwrap();
        current_state.version_string = text_buffer.clone();

        // --- 3. Comparison and Drawing Phase (The "Diffing" Engine) ---
        // ONLY draw if the string has changed since the last frame.
        

        // Header
        if current_state.header_string != previous_state.header_string {
            display_controller.draw_text_line(&current_state.header_string, Point::new(20, 30), &PROFONT_14_POINT, Rgb565::CSS_ROSY_BROWN, Rgb565::BLACK)?;
        }
        if current_state.axis_x_string != previous_state.axis_x_string {
            display_controller.draw_text_line(&current_state.axis_x_string, Point::new(20, 50), &PROFONT_14_POINT, Rgb565::WHITE, Rgb565::BLACK)?;
        }
        if current_state.axis_y_string != previous_state.axis_y_string {
            display_controller.draw_text_line(&current_state.axis_y_string, Point::new(20, 70), &PROFONT_14_POINT, Rgb565::WHITE, Rgb565::BLACK)?;
        }
        if current_state.axis_z_string != previous_state.axis_z_string {
            display_controller.draw_text_line(&current_state.axis_z_string, Point::new(20, 90), &PROFONT_14_POINT, Rgb565::WHITE, Rgb565::BLACK)?;
        }
        if current_state.status_string != previous_state.status_string {
            display_controller.draw_text_line(&current_state.status_string, Point::new(20, 110), &PROFONT_14_POINT, Rgb565::WHITE, Rgb565::BLACK)?;
        }
        
        if current_state.gps_string != previous_state.gps_string {
            display_controller.draw_text_line(&current_state.gps_string, Point::new(20, 150), &PROFONT_14_POINT, Rgb565::GREEN, Rgb565::BLACK)?;
        }
        if current_state.utc_string != previous_state.utc_string {
            display_controller.draw_text_line(&current_state.utc_string, Point::new(20, 170), &PROFONT_14_POINT, Rgb565::YELLOW, Rgb565::BLACK)?;
        }
        if current_state.lst_string != previous_state.lst_string {
            display_controller.draw_text_line(&current_state.lst_string, Point::new(20, 190), &PROFONT_14_POINT, Rgb565::YELLOW, Rgb565::BLACK)?;
        }
        if current_state.lon_string != previous_state.lon_string {
            display_controller.draw_text_line(&current_state.lon_string, Point::new(20, 210), &PROFONT_14_POINT, Rgb565::WHITE, Rgb565::BLACK)?;
        }
        if current_state.lat_string != previous_state.lat_string {
            display_controller.draw_text_line(&current_state.lat_string, Point::new(20, 230), &PROFONT_14_POINT, Rgb565::WHITE, Rgb565::BLACK)?;
        }
        if current_state.webserver_string != previous_state.webserver_string {
            display_controller.draw_text_line(&current_state.webserver_string, Point::new(20, 270), &PROFONT_14_POINT, Rgb565::WHITE, Rgb565::BLACK)?;
        }
        if current_state.version_string != previous_state.version_string {
            display_controller.draw_text_line(&current_state.version_string, Point::new(20, 290), &PROFONT_14_POINT, Rgb565::GREEN, Rgb565::BLACK)?;
        }
        if current_state.builtby_string != previous_state.builtby_string {
            display_controller.draw_text_line(&current_state.builtby_string, Point::new(20, 310), &PROFONT_14_POINT, Rgb565::GREEN, Rgb565::BLACK)?;
        }
        
        
        // --- 4. Update State for Next Frame ---
        // The current state now becomes the previous state for the next loop iteration.
        previous_state = current_state;


        loop_counter += 1;
        // Print heap status every 30 seconds (e.g., if your loop delay is 200ms, 150 loops = 30s)
        if loop_counter % 150 == 0 {
            // This sys call gets the amount of free memory in bytes.
            let free_heap = unsafe { esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_DEFAULT) };
            let stack_hwm = unsafe { uxTaskGetStackHighWaterMark(main_task_handle) };
            

            // Log all system monitor data.
            log::info!(
                "[SYS_MONITOR] Main Task - Free Heap: {} bytes, Stack HWM: {} bytes",
                free_heap,
                stack_hwm
            );
            
            
        }
        
        // --- 5. Control the Refresh Rate ---
        FreeRtos::delay_ms(100); // We can even update faster now (10Hz)

        
    }
}
#[cfg(not(feature = "display"))]
    {
        println!("Display feature is DISABLED. Running in headless mode.");
        //hardware_wiring_test(system)?;
        
        // In headless mode, the main task has very little to do. It just needs
        // to stay alive and can perform periodic system monitoring.
        let main_task_handle = unsafe { xTaskGetCurrentTaskHandle() };
        let mut loop_counter = 0;
        
        loop {
            
            // We can still have our system monitor.
            if loop_counter % 150 == 0 {
                let free_heap = unsafe { esp_idf_svc::sys::heap_caps_get_free_size(esp_idf_svc::sys::MALLOC_CAP_DEFAULT) };
            let stack_hwm = unsafe { uxTaskGetStackHighWaterMark(main_task_handle) };
                log::info!(
                    "[SYS_MONITOR] Main Task (Headless) - Free Heap: {}, Stack HWM: {}",
                    free_heap, stack_hwm
                );
                // --- NEW DIAGNOSTIC LOG ---
              let last_byte = {
                  // We need to access the planner to call the getter.

                use esp_idf_svc::hal::task::CriticalSection;

                
                  let _cs = CriticalSection::new();
                  let planner = unsafe { &*system.planner.0 };
                  planner.get_last_step_byte() // Assuming you add this getter
              };

              log::info!(
                  "[STEP_MONITOR] Last step pulse byte sent: {:#010b}", // The #010b formats as `0b00000000`
                  last_byte
              );
            }
            loop_counter += 1;
            
            // We can sleep for longer in headless mode, as there's no UI to update.
            FreeRtos::delay_ms(100);
        }
    }
}
