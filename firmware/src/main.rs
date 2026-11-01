// src/main.rs

mod gps;
mod stepper;
mod storage;
mod time;
//mod webserver;
mod wifi;
mod systemsvcs;


mod mount;

use std::fs::{read_dir, File};
use std::io::Read;
use std::sync::{Arc, Mutex};

use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::sys::{uxTaskGetStackHighWaterMark, xTaskCreatePinnedToCore, xTaskGetCurrentTaskHandle};

use log::{info};

use crate::mount::alpacaserver::alpaca_task_entry;
use crate::mount::{ commander_task_entry, RustyScope, RustyScopeState };
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
