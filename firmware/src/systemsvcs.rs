// src/systemsvcs.rs
use anyhow::Result;
use hifitime::{Epoch, TimeScale};

use std::net::Ipv4Addr;
use std::sync::Arc;

use esp_idf_svc::fs::fatfs::Fatfs;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::{self, AnyIOPin, Gpio0, Gpio1};
use esp_idf_svc::hal::sd::spi::SdSpiHostDriver;
use esp_idf_svc::hal::sd::{SdCardConfiguration, SdCardDriver};
use esp_idf_svc::hal::spi::config::DriverConfig;
use esp_idf_svc::hal::spi::{Dma, SpiDriver};
// use esp_idf_svc::http::server::EspHttpServer;
use esp_idf_svc::hal::uart::{UartConfig, UartDriver};
use esp_idf_svc::io::vfs::MountedFatfs;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::{esp_timer_create, esp_timer_create_args_t, esp_timer_handle_t, esp_timer_start_periodic};
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::{eventloop::EspSystemEventLoop, hal::peripherals::Peripherals};
use esp_idf_svc::hal::units::Hertz;
use log::{error, info};


use crate::gps::pps_sync;
use crate::stepper::{BitbangStepperDriver};
use crate::{
    gps::GpsController, 
    ntp::NtpServer,
    stepper::MotionPlanner, 
    storage::StorageManager, 
    time::TimeManager, 
    // webserver::{config_webserver, create_webserver},
    wifi::wifi_create,
};

#[cfg(feature = "display")]
use crate::ts35::DisplayController;


#[cfg(feature = "display")]
// This struct hold all the high-level, thread-safe application services.
pub struct SystemServices<'d> {
    // --- Hardware Service Resources ---
    pub planner: Arc<Mutex<MotionPlanner<'d>>>,
    pub gps: Arc<GpsController>,
    pub time: Arc<TimeManager>,
    pub display: Arc<Mutex<DisplayController<'d>>>,
    pub _sdcard: Option<SendWrapper<MountedFatfs<Fatfs<SdCardDriver<SdSpiHostDriver<'d, SpiDriver<'d>>>>>>>,
    pub spiffs:  Option<StorageManager>,
    // --- Service Resources ---
    _wifi: SendWrapper<Box<EspWifi<'d>>>,
    _webserver: Option<SendWrapper<EspHttpServer<'d>>>,
    pub ipaddr: Ipv4Addr,
    pub mac_string: String,
}

#[cfg(not(feature = "display"))]
pub struct SystemServices<'d> {
    // --- Hardware Service Resources ---
    //pub planner: Arc<Mutex<MotionPlanner<'d>>>,
    pub planner: SendWrapper<*mut MotionPlanner<'d>>,
    pub gps: Arc<GpsController>,
    pub time: Arc<TimeManager>,
    // pub display: Arc<Mutex<DisplayController<'d>>>,
    pub _sdcard: Option<SendWrapper<MountedFatfs<Fatfs<SdCardDriver<SdSpiHostDriver<'d, SpiDriver<'d>>>>>>>,
    pub spiffs:  Option<StorageManager>,
    // --- Service Resources ---
    _wifi: SendWrapper<Box<EspWifi<'d>>>,
    //_webserver: Option<SendWrapper<EspHttpServer<'d>>>,
    pub ipaddr: Ipv4Addr,
    pub mac_string: String,
}

impl<'d> SystemServices<'d> {
    pub fn new() -> Result<Self> {
        // --- Core system resource list ---
        let peripherals = Peripherals::take().unwrap();
        let sys_loop = EspSystemEventLoop::take()?;
        let nvs = EspDefaultNvsPartition::take()?;
        let pins = peripherals.pins;


        // --- Wifi setup ---
        let wifi = SendWrapper(wifi_create(peripherals.modem, sys_loop, nvs)?);
        let ipaddr = wifi.0.sta_netif().get_ip_info().unwrap().ip;
        let mac_addr = wifi.0.sta_netif().get_mac().unwrap();
        let mac_string = mac_addr
            .iter()
            .map(|byte| format!("{:02X}", byte))
            .collect::<String>();
    
        println!("[SystemServices] Wifi connected! IP: {}, MAC: {}", ipaddr, mac_string);

        // --- Spiffs Storage Initialization ---
        info!("[SystemServices] Initializing SPIFFS storage...");
        let storage_manager: Option<StorageManager> = Some(StorageManager::new()?);

        // --- --- Load Configuration from SPIFFS---
        // let mut storageconfig = storage_manager.load_config()?;
        // info!("Loaded configuration: {:?}", storageconfig);

        // --- SD CARD INITIALIZATION:  ---
        info!("[SystemServices] Initializing SD card...");
        let mounted_fatfs_opt  = {
            let result = (|| -> anyhow::Result<MountedFatfs<_>, anyhow::Error> {
                // Line 1: Create the SPI Driver
                let spi2_driver = SpiDriver::new(
                    peripherals.spi2,
                    pins.gpio14, // SCK
                    pins.gpio13, // MOSI
                    Some(pins.gpio12), // MISO
                &DriverConfig::default().dma(Dma::Auto(4096)),
                )?;
        
                // Line 2: Create the SD Card Driver
                let sd_card_driver = SdCardDriver::new_spi(
                    SdSpiHostDriver::new(
                        spi2_driver,
                        Some(pins.gpio15), // CS
                        Some(pins.gpio39), // Detect
                        AnyIOPin::none(),
                        AnyIOPin::none(),
                        None,
                    )?,
                    &SdCardConfiguration::new(),
                )?;
        
                // Line 3: Mount the filesystem and keep the guard object alive.
                let mounted_fatfs = MountedFatfs::mount(
                    Fatfs::new_sdcard(0, sd_card_driver)?, 
                   "/sdcard", 
                 4 // Max open files
                )?;
                Ok(mounted_fatfs)
            })();
    
             match result {
                Ok(mounted_fs) => {
                    info!("[SystemServices] SD Card mounted successfully at /sdcard");
                    Some(SendWrapper(mounted_fs))
                }
                Err(e) => {
                    error!("[SystemServices] Failed to initialize SD card: {}. Continuing without SD card.", e);
                    None
                }
            }
        };

        /*  DISABLED FOR NOW - works, but "there can only be one" - Highlander movie
        // --- Webserver setup and startup ---
        // 1. Declare the server variable in the main scope, initialized to None.
        //    It needs to be mutable because we will assign to it later.
        let mut webserver = None;
    
     
        // 2. The `if let` block now assigns to the outer `server` variable.
        if let Some(_sdcard_guard) = &mounted_fatfs_opt {
            // info!("[SystemServices] SD Card found, starting web server...");
            
            // Create a temporary server instance.
            let mut temp_server = create_webserver()?;
            // Configure it.
            config_webserver(&mut temp_server)?;
            
            // Move the configured server into our `Option`.
            webserver = Some(SendWrapper(temp_server));
            
            info!("[SystemServices] Web server started. Open http://{} in your browser.", ipaddr);
    
        } else {
            error!("[SystemServices] No SD Card found, web server will not be started.");
        }; 
        */

        // Initialize GPS
        let tx_pin = pins.gpio4;
        let rx_pin = pins.gpio0;
        let gpsconfig = UartConfig::new()
            .baudrate(Hertz(38400))
            .data_bits(esp_idf_svc::hal::uart::config::DataBits::DataBits8)
            .parity_none()
            .stop_bits(esp_idf_svc::hal::uart::config::StopBits::STOP1);
        let gpsuart = UartDriver::new(
            peripherals.uart2,
            tx_pin,
            rx_pin,
            Option::<Gpio0>::None,
            Option::<Gpio1>::None,
            &gpsconfig,
        )?;
        // ... and fire up GPS controller
        let gps = GpsController::new(gpsuart)?;

        // --- PPS Initialization ---
        // Enable the global GPIO ISR service. This must be done once per application.
        // The new API takes no arguments and returns a Result.
        gpio::enable_isr_service()?;
    
        // Now, initialize our PPS input pin and register the handler.
        // Make sure you're using the correct pin for your hardware.
        pps_sync::init_pps_input(pins.gpio22)?;
    
        // --- Time Manager Initialization ---
        // The TimeManager needs the GpsController to get its position.
        info!("[SystemServices] Initializing Time Manager...");
        let time_manager = TimeManager::new(gps.clone()); // This spawns its own task.
        //info!("[SystemServices] Time Manager started.");

        // setting up the GPS time synchronization
        let mut time_has_been_set = false;
    
        loop {
            // We can check if we need to set the time.
            if !time_has_been_set {
                if gps.has_fix() {
                    // We have a fix, let's try to set the system time.
                    match gps.set_system_time() {
                        Ok(true) => {
                            // It worked! We don't need to do this again.
                            time_has_been_set = true;
                            // You can now log the current system time to verify
                            let now = Epoch::now()?;
                            info!("[SystemServices] Verification: System time is now {:?}", now.to_gregorian_str(TimeScale::TAI));
                            break; // Exit the loop since we set the time successfully.
                        }
                        Ok(false) => {
                            // This means we have a position fix ($GPGGA) but haven't
                            // received a time sentence yet ($GPRMC). We'll try again later.
                            info!("[SystemServices] Have GPS fix, but waiting for time data to sync clock...");
                        }
                        Err(e) => {
                            // This indicates a more serious system error.
                            error!("[SystemServices] Error setting system time: {}", e);
                        }
                    }
                } else {
                    info!("[SystemServices] Waiting for GPS fix before setting time...");
                    FreeRtos::delay_ms(1500);
                }
            }
        }
        // --- Add a small loop to wait for the first valid LST calculation ---
        info!("[SystemServices] Waiting for first valid LST calculation...");
        let mut current_lst;
        loop {
            current_lst = time_manager.get_lst();
            if current_lst > 0.0 {
                info!("[SystemServices] LST is valid: {:.4} hours. Proceeding...", current_lst);
                break;
            }
            // Also check if GPS time has been set, as LST depends on it.
            if time_has_been_set { // Assuming you have this flag from the time-setting logic
                 // If time is set but LST is still 0, the time_task just needs a moment to run.
            }
            FreeRtos::delay_ms(500);
        }
        // ---------------- NTP server ----------------
        // Shares the controller and keeps port :123 alive in background threads.
        let _ntp = NtpServer::start(gps.clone());
        info!("NTP server started on UDP {ipaddr}:123");
        info!("Try testing with: $ ntpdate -q {ipaddr}");
        
        const TIMER_FREQUENCY_HZ: u64 = 5_000;
        

        
        info!("[SystemServices] Initializing Bit-Banged Stepper Driver...");
        let bitbang_driver = BitbangStepperDriver::new(
            pins.gpio16, // SCK - Serial Clock
            pins.gpio21, // MOSI - Serial Data
            pins.gpio17, // LATCH - Latch Clock
        )?;
        info!("[SystemServices] Bit-Banged Driver Initialized.");

        // Create the Planner instance.
        
        let planner = MotionPlanner::new(bitbang_driver, TIMER_FREQUENCY_HZ);
        let planner_box = Box::new(planner);
        let planner_ptr = Box::into_raw(planner_box);

        // --- Setup the High-Resolution Timer ---
        println!("[SystemServices] Setting up high-resolution motion timer...");
        let create_args = esp_timer_create_args_t {
            callback: Some(timer_isr_callback),
            //arg: planner_arc_ptr, // Pass the raw pointer to the planner Arc
            arg: planner_ptr as *mut _,
            //dispatch_method: esp_idf_svc::sys::esp_timer_dispatch_t_ESP_TIMER_MAX,
            //dispatch_method: esp_timer_dispatch_t::ESP_TIMER_MAX,
            dispatch_method: 1,
            // name: "motion_timer".as_ptr() as *const u8,
            name: "motion_timer".as_ptr(),
            skip_unhandled_events: true,
        };

        let mut timer_handle: esp_timer_handle_t = core::ptr::null_mut();
        // For a system like this, a timer frequency between 1 kHz and 10 kHz is typically the sweet spot.
        // 1 kHz (period_us = 1000): Very safe, low CPU load. Good for initial testing. Might feel a bit slow at the top end.
        // 2 kHz (period_us = 500): A good balance between responsiveness and CPU load. Good for most applications.
        // 3 kHz (period_us = 333): A bit more CPU load, but still manageable. Good for applications that require high precision.
        // 4 kHz (period_us = 250): A bit more CPU load, but still manageable. Good for applications that require high precision.
        // 5 kHz (period_us = 200): A great, aggressive target. Provides excellent speed and resolution while still being very manageable for the ESP32 CPU.
        // 10 kHz (period_us = 100): Likely the upper limit of what is necessary. Provides very fine control but starts to consume a noticeable amount of CPU time.
        let period_us = 1_000_000 / TIMER_FREQUENCY_HZ; // 200 microseconds = 5 kHz
        unsafe {
            esp_timer_create(&create_args, &mut timer_handle);
            esp_timer_start_periodic(timer_handle, period_us);
        }
        println!("[SystemServices] Motion timer started with a {} us period ({} Hz).", period_us, 1_000_000 / period_us);
    
    
        #[cfg(feature = "display")]
        let display_controller = {
        // --- Initialize the display controller ---
        // This is the new display controller that manages both the display and touch.
        let spi_config = SpiDriverConfig::new().dma(Dma::Auto(4096));
        let spi3_driver = SpiDriver::new(
            peripherals.spi3,
            pins.gpio18, // CLK
            pins.gpio23, // MOSI
            Some(pins.gpio19), // MISO
            &spi_config,
        )?;
        // Create the high-level controller, passing it all the resources it needs.
        let controller = Arc::new(Mutex::new(DisplayController::new(
            spi3_driver,
            pins.gpio25, // LCD_CS
            pins.gpio26, // TOUCH_CS
            pins.gpio27, // LCD_RST
            pins.gpio33, // LCD_DC
            pins.gpio5,  // Backlight Enable
        )?));

        println!("[SystemServices] Display controller initialized.");
        println!("[SystemServices] Showing splash screen");
        let _ = controller.lock().unwrap().clear_screen()?;
        // --- Display the Splash Screen ---
        // The path is relative to the root of the filesystem, which VFS handles.
        match controller.lock().unwrap().display_splash_16bit("/sdcard/qwen2.bmp") {
            Ok(_) => info!("Splash screen displayed."),
            Err(e) => error!("Could not display splash screen: {}", e),
        }
        Some(controller)
    };

    #[cfg(not(feature = "display"))]
        info!("[SystemServices] Display feature disabled, running in headless mode.");
    

        // --- Conditionally construct and return the correct struct ---

        // This is the return block for when the "display" feature is ON.
        #[cfg(feature = "display")]
        {
            Ok(Self {
                planner: shared_planner,
                gps: gps,
                time: time_manager,
                display: display_controller.unwrap(), // We know it's Some
                _sdcard: mounted_fatfs_opt,
                spiffs: storage_manager,
                _wifi: wifi,
                _webserver: webserver,
                ipaddr: ipaddr,
                mac_string: mac_string,
            })
        }

        // This is the return block for when the "display" feature is OFF.
        // It's identical, but it doesn't try to initialize the `display` field.
        #[cfg(not(feature = "display"))]
        {
            Ok(Self {
                //planner: shared_planner,
                planner: SendWrapper(planner_ptr),
                gps,
                time: time_manager,
                // No `display` field here
                _sdcard: mounted_fatfs_opt,
                spiffs: storage_manager,
                _wifi: wifi,
                //_webserver: webserver,
                ipaddr,
                mac_string,
            })
        }

        

    }
}

// --- This is our new ISR callback function ---
// It must have the `extern "C"` signature to be callable from C code.
// This attribute tells the linker to place the machine code for this entire
// function into the IRAM section of the ESP32's memory.
#[inline(never)]
#[link_section = ".iram1.text"]
extern "C" fn timer_isr_callback(arg: *mut core::ffi::c_void) {
    // The ISR now gets a raw pointer to the planner itself.
    let planner = unsafe { &mut *(arg as *mut MotionPlanner) };
    // No lock needed! The ISR has exclusive access during its execution.
    let _ = planner.update();
}


// A wrapper to make a non-Send type Send.
// This is an `unsafe` contract where we promise to only access it safely.
pub struct SendWrapper<T>(pub T);
unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {} // Also implement Sync for references
