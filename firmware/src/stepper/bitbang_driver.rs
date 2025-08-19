// src/stepper/bitbang_driver.rs

use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{AnyIOPin, Output, PinDriver};
use esp_idf_svc::sys::EspError;

/// A simple, software-driven ("bit-banged") driver for the 74HC595 shift register.
/// This gives us absolute control over the timing of the data, clock, and latch signals.
pub struct BitbangStepperDriver<'d> {
    sclk: PinDriver<'d, AnyIOPin, Output>,  // The Serial Clock (SH_CP)
    mosi: PinDriver<'d, AnyIOPin, Output>,  // The Serial Data (DS)
    latch: PinDriver<'d, AnyIOPin, Output>, // The Latch Clock (ST_CP)
}

impl<'d> BitbangStepperDriver<'d> {
    pub fn new(
        sclk: impl Into<AnyIOPin>,
        mosi: impl Into<AnyIOPin>,
        latch: impl Into<AnyIOPin>,
    ) -> Result<Self, EspError> {
        // Initialize all pins as outputs and set them to a known low state.
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
    
    /// Sends a single byte to the shift register.
    #[inline(never)]
    #[link_section = ".iram1.text"]
    fn write_byte(&mut self, byte: u8) -> Result<(), EspError> {
        // Iterate through each of the 8 bits in the byte, from most significant to least.
        for i in (0..8).rev() {
            // 1. Set the data pin (MOSI) to the value of the current bit.
            if (byte >> i) & 1 == 1 {
                self.mosi.set_high()?;
            } else {
                self.mosi.set_low()?;
            }

            // 2. Pulse the clock pin (SCLK) to shift the bit into the register.
            self.sclk.set_high()?;
            // A tiny delay is needed to meet the chip's timing requirements. Ets is IRAM-safe.
            Ets::delay_us(1); 
            self.sclk.set_low()?;
        }
        Ok(())
    }

    /// This is the main function called by the planner. It sends a buffer of bytes.
    // Corrected send_steps for our use case

    #[inline(never)]
    #[link_section = ".iram1.text"]
    pub fn send_steps(&mut self, buffer: &[u8]) -> Result<(), EspError> {
        // We expect a buffer of [step_high, step_low]
        // We will send the step_high byte first, then the step_low byte.
        // We only latch after the final byte to ensure a clean transition.

        // Send the "high" pulse byte.
        if let Some(byte) = buffer.get(0) {
            self.write_byte(*byte)?;
            self.latch.set_high()?;
            Ets::delay_us(1);
            self.latch.set_low()?;
        }
        
        // Send the "low" pulse byte.
        if let Some(byte) = buffer.get(1) {
            self.write_byte(*byte)?;
            self.latch.set_high()?;
            Ets::delay_us(1);
            self.latch.set_low()?;
        }
        
        Ok(())
    }
}