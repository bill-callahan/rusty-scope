// src/gps/pps_sync.rs

//! GPS Pulse-Per-Second (PPS) Synchronization Module
//!
//! This module is responsible for disciplining the ESP32's internal high-resolution
//! timer against the highly accurate PPS signal provided by a GPS module.
//!
//! It works by setting up a GPIO interrupt that fires on the rising edge of the
//! PPS pulse. The ISR (Interrupt Service Routine) measures the precise interval
//! between pulses using the ESP32's `esp_timer`. By comparing this measured interval
//! against the ideal interval of 1,000,000 microseconds, it calculates a real-time
//! "clock correction factor".
//!
//! This factor can then be used by other parts of the system, such as the sidereal
//! tracking logic, to compensate for crystal drift caused by temperature changes
//! or manufacturing tolerances, resulting in extremely high-precision tracking over
//! long periods.

use esp_idf_svc::hal::gpio::{self, InputPin, InterruptType, OutputPin, PinDriver, Pull};
use esp_idf_svc::sys::{esp_timer_get_time, gpio_isr_handler_add, EspError, ESP_OK};
use log::info;
use portable_atomic::{AtomicF64, AtomicU64, Ordering};


// --- Global, thread-safe (atomic) variables ---

/// Stores the timestamp (in microseconds from `esp_timer_get_time()`) of the last PPS pulse.
/// This is accessed by the ISR and must be atomic.
static LAST_PPS_TIMESTAMP: AtomicU64 = AtomicU64::new(0);
static PPS_COUNT: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);

/// The calculated clock correction factor.
/// This value is the core output of this module.
/// - `1.0` means the ESP32 clock is perfectly accurate.
/// - `> 1.0` means the ESP32 clock is running slightly slow.
/// - `< 1.0` means the ESP32 clock is running slightly fast.
/// 
/// We use `portable_atomic::AtomicF64` because the standard library lacks an atomic float.
static RAW_CORRECTION_FACTOR: AtomicF64 = AtomicF64::new(1.0);
static FILTERED_CORRECTION_FACTOR: AtomicF64 = AtomicF64::new(1.0);

/// The GPIO interrupt handler that is called on every PPS pulse.
///
/// This function must have the `extern "C"` ABI to be callable from the ESP-IDF's C code.
/// It is designed to be extremely fast and lean, performing only essential calculations.
#[doc(hidden)]
extern "C" fn pps_isr_handler(_arg: *mut core::ffi::c_void) {
    PPS_COUNT.fetch_add(1, portable_atomic::Ordering::SeqCst);
    // Get the current high-resolution timestamp from the ESP32's timer.
    // This is an `unsafe` call as it's a direct FFI to the C SDK.
    let now_us = unsafe { esp_timer_get_time() as u64 };

    // Atomically replace the old timestamp with the new one and get the old value back.
    // This is a single, atomic Read-Modify-Write operation.
    let last_us = LAST_PPS_TIMESTAMP.swap(now_us, Ordering::SeqCst);

    // If last_us is 0, this is the first pulse we've received, so we can't calculate a delta yet.
    if last_us > 0 {
        let delta_us = now_us - last_us;

        // The ideal, perfect interval between PPS pulses is exactly 1 second.
        const ONE_SECOND_US: f64 = 1_000_000.0;

        // --- NEW: Outlier Rejection ---
        // We expect the delta to be very close to 1 second. If it's off by more
        // than a reasonable amount (e.g., 5%), we assume an interrupt was missed
        // or delayed, and we discard this measurement. 5% = 50,000 us.
        if delta_us > 950_000 && delta_us < 1_050_000 {
            let raw_factor = ONE_SECOND_US / delta_us as f64;
            RAW_CORRECTION_FACTOR.store(raw_factor, Ordering::SeqCst);

            // --- IIR Low-Pass Filter ---
            // We can make the filter a bit "stronger" by reducing ALPHA.
            const ALPHA: f64 = 0.05; // Was 0.1, now 0.05 for more smoothing.

            let old_filtered = FILTERED_CORRECTION_FACTOR.load(Ordering::Relaxed);
            let new_filtered = (ALPHA * raw_factor) + ((1.0 - ALPHA) * old_filtered);
            
            FILTERED_CORRECTION_FACTOR.store(new_filtered, Ordering::SeqCst);
        } else {
            // This is a rare event, but we can log it if we want to from a safe context later.
            // For now, we just ignore the bad measurement. The next pulse will be fine.
            // The LAST_PPS_TIMESTAMP is already updated, so the next delta will be correct.
        }
        
    }
}

/// Initializes the specified GPIO pin as an input for the PPS signal and registers
/// the interrupt handler. This must be called once during system startup.
///
/// # Safety
/// This function relies on the global GPIO ISR service being installed first.
/// It also uses `core::mem::forget` to prevent the `PinDriver` from being dropped,
/// which would de-register the interrupt.
pub fn init_pps_input(pin: impl gpio::Pin + InputPin + OutputPin) -> Result<(), EspError> {
    info!("Initializing GPS PPS input on GPIO {}...", pin.pin());

    // Configure the pin as an input with a pull-down resistor and enable the interrupt
    // on the rising edge of the pulse.
    let mut pps_pin = PinDriver::input(pin, Pull::Down)?;
    //pps_pin.set_pull(Pull::Down)?;
    //pps_pin.set_interrupt_type(InterruptType::RisingEdge)?;
    pps_pin.set_interrupt_type(InterruptType::PosEdge)?;

    // The `unsafe` block is required for the FFI call to `gpio_isr_handler_add`.
    // We are guaranteeing that `pps_isr_handler` has the correct C signature and
    // will be valid for the lifetime of the program.
    unsafe {
        let result = gpio_isr_handler_add(
            pps_pin.pin().into(),
            Some(pps_isr_handler),
            core::ptr::null_mut(), // No argument is passed to the ISR
        );

        if result != ESP_OK {
            return Err(EspError::from(result).unwrap());
        }
    }

    // This is a crucial step. We must "leak" the `PinDriver`. If we let it go out
    // of scope, its `Drop` implementation would run and automatically de-register
    // the interrupt handler we just set up. `mem::forget` prevents this.
    core::mem::forget(pps_pin);

    info!("PPS ISR handler registered successfully.");
    Ok(())
}

/// A thread-safe public function for other modules to get the SMOOTHED clock correction factor.
pub fn get_clock_correction_factor() -> f64 {
    FILTERED_CORRECTION_FACTOR.load(Ordering::SeqCst)
}

pub fn last_pps_timestamp_us() -> u64 {
    LAST_PPS_TIMESTAMP.load(portable_atomic::Ordering::SeqCst)
}

pub fn pps_pulse_count() -> u64 {
    PPS_COUNT.load(portable_atomic::Ordering::SeqCst)
}

pub fn clock_correction_factor() -> f64 {
    get_clock_correction_factor()
}