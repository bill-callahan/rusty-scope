// src/stepper/bitbang_driver.rs

use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{AnyIOPin, Output, PinDriver};
use esp_idf_svc::sys::EspError;

/// A software-driven ("bit-banged") driver for the 74HC595 shift register.
/// 
/// This driver gives us absolute, cycle-accurate control over the timing of the data, 
/// clock, and latch signals. This is essential for generating reliable step pulses 
/// for stepper motor drivers like the LV8729.
/// 
/// # Hardware Mapping
/// * `sclk` -> SH_CP (Shift Register Clock Pin)
/// * `mosi` -> DS (Serial Data Input)
/// * `latch` -> ST_CP (Storage Register Clock / Latch Pin)
pub struct BitbangStepperDriver<'d> {
    sclk: PinDriver<'d, Output>,  // The Serial Clock (SH_CP)
    mosi: PinDriver<'d, Output>,  // The Serial Data (DS)
    latch: PinDriver<'d, Output>, // The Latch Clock (ST_CP)
}

impl<'d> BitbangStepperDriver<'d> {
    /// Initializes the GPIO pins as outputs and sets them to a known low state.
    pub fn new(
        sclk: impl Into<AnyIOPin<'static>>,
        mosi: impl Into<AnyIOPin<'static>>,
        latch: impl Into<AnyIOPin<'static>>,
    ) -> Result<Self, EspError> {
        let mut sclk_driver = PinDriver::output(sclk.into())?;
        let mut mosi_driver = PinDriver::output(mosi.into())?;
        let mut latch_driver = PinDriver::output(latch.into())?;

        sclk_driver.set_low()?;
        mosi_driver.set_low()?;
        latch_driver.set_low()?;

        Ok(Self {
            sclk: sclk_driver,
            mosi: mosi_driver,
            latch: latch_driver,
        })
    }
    
    /// Shifts a single byte into the 74HC595 register, MSB first.
    /// 
    /// # Timing Note
    /// Includes a 1µs setup delay before the clock edge to easily meet the 74HC595's 
    /// data setup time (Tds) requirements at 3.3V. Because the ESP32 executes instructions 
    /// so quickly, the data pin might change state only nanoseconds before the clock rises 
    /// without this delay, causing bit-shifting errors.
    #[inline(never)]
    #[link_section = ".iram1.text"]
    fn write_byte(&mut self, byte: u8) -> Result<(), EspError> {
        for i in (0..8).rev() {
            // 1. Set the data pin (MOSI) to the value of the current bit.
            if (byte >> i) & 1 == 1 {
                self.mosi.set_high()?;
            } else {
                self.mosi.set_low()?;
            }

            // 2. Small delay to ensure Data Setup Time (t_su) is met before clock rises.
            Ets::delay_us(1); 

            // 3. Pulse the clock pin (SCLK) to shift the bit into the register.
            self.sclk.set_high()?;
            Ets::delay_us(1); // Ensure minimum clock high pulse width (T_w)
            self.sclk.set_low()?;
        }
        Ok(())
    }

    /// Sends a sequence of bytes to the shift register to generate a step pulse.
    /// 
    /// # Arguments
    /// * `buffer` - A slice containing the byte states to output. For a standard step pulse 
    ///   on a single 74HC595, this is expected to be `[step_high_state, step_low_state]`.
    ///
    /// # Logic
    /// To create a pulse on a single shift register, we must shift in the HIGH state and 
    /// latch it (updating the physical outputs). Then, we shift in the LOW state and latch 
    /// it again. This sequence creates a clean hardware pulse on the output pins.
    #[inline(never)]
    #[link_section = ".iram1.text"]
    pub fn send_steps(&mut self, buffer: &[u8]) -> Result<(), EspError> {
        // 1. Send the "high" pulse byte and latch to update outputs immediately.
        if let Some(&byte) = buffer.first() {
            self.write_byte(byte)?;
            self.latch.set_high()?;
            Ets::delay_us(1); // Ensures minimum latch pulse width
            self.latch.set_low()?;
        }
        
        // 2. Send the "low" pulse byte and latch to complete the step pulse.
        if let Some(&byte) = buffer.get(1) {
            self.write_byte(byte)?;
            self.latch.set_high()?;
            Ets::delay_us(1);
            self.latch.set_low()?;
        }
        
        Ok(())
    }
}