// src/stepper/i2s0.rs

use esp_idf_svc::hal::delay::{Ets, FreeRtos};
use esp_idf_svc::hal::gpio::{AnyIOPin, Output, PinDriver};
use esp_idf_svc::hal::i2s::config::{Config, Role, SlotMode, StdClkConfig, StdGpioConfig, StdSlotConfig, StdSlotMask};
use esp_idf_svc::hal::i2s::{
    config::{DataBitWidth, StdConfig},
    I2sDriver, I2sTx,
};
use esp_idf_svc::hal::i2s::I2S0;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::sys::{i2s_channel_fmt_t_I2S_CHANNEL_FMT_ONLY_LEFT, i2s_comm_format_t_I2S_COMM_FORMAT_I2S_LSB, i2s_comm_format_t_I2S_COMM_FORMAT_I2S_MSB, i2s_comm_format_t_I2S_COMM_FORMAT_STAND_I2S, i2s_config_t, i2s_mclk_multiple_t_I2S_MCLK_MULTIPLE_128, i2s_mode_t_I2S_MODE_MASTER, i2s_mode_t_I2S_MODE_TX, EspError};

/// I2S Stepper Driver using Standard Transmit mode.
pub struct I2sStepperDriver<'d> {
    i2s_tx: I2sDriver<'d, I2sTx>,
    isr_buffer: [u8; 4], // Maximum size we'll need for 2 bytes of data
}

impl<'d> I2sStepperDriver<'d> {
    /// Creates a new I2S stepper driver.
    ///
    /// # Arguments
    ///
    /// * `i2s` - The I2S0 peripheral.
    /// * `bck` - The pin for the I2S Bit Clock (connects to Shift Register SH_CP).
    /// * `ws` - The pin for the I2S Word Select (connects to Shift Register ST_CP/Latch).
    /// * `data` - The pin for I2S Data Out (connects to Shift Register DS).
    /// * `sample_rate` - The rate at which bytes are sent.
    pub fn new(
        i2s: I2S0,
        bck: impl Into<AnyIOPin>,  //Real BCK pin
        data: impl Into<AnyIOPin>,  //Real Data pin
        latch: impl Into<AnyIOPin>,  // Latch pin
        sample_rate: Hertz,
    ) -> Result<Self, EspError> {
       
       let config = Config::default().role(Role::Controller);
       let clk_config = StdClkConfig::from_sample_rate_hz(sample_rate.into());
       let slot_config = StdSlotConfig::msb_slot_default(DataBitWidth::Bits8, SlotMode::Mono)
           .slot_mode_mask(SlotMode::Mono, StdSlotMask::Left)
           ;
       let gpio_config = StdGpioConfig::default();

       let std_config = StdConfig::new(config, clk_config, slot_config, gpio_config);
    



        // The pin order is: bclk, dout, din (None for us), ws
        let mut driver = I2sDriver::new_std_tx(
            i2s,
            &std_config,
            bck.into(),
            data.into(),
            Option::<AnyIOPin>::None, // We are not using a data-in pin
            latch.into(),
        )?;

        driver.tx_enable()?;

        
        Ok(Self {
            i2s_tx: driver,
            isr_buffer: [0; 4],
        })
    }    
    #[link_section = ".iram1.text"]
    /// Writes a buffer of step commands to the I2S DMA engine.    
    pub fn send_steps(&mut self, buffer: &[u8]) -> Result<(), EspError> {
        // Use the pre-allocated buffer to avoid heap allocation
        // This is safe because we're in a single-threaded ISR context
        let formatted_buffer = &mut self.isr_buffer;
        
        // Format the data directly in the pre-allocated buffer
        if let Some(&byte1) = buffer.get(0) {
            formatted_buffer[0] = byte1; // High byte of first 16-bit word
            formatted_buffer[1] = 0;    // Low byte of first 16-bit word (ignored)
        }
        if let Some(&byte2) = buffer.get(1) {
            formatted_buffer[2] = byte2; // High byte of second 16-bit word
            formatted_buffer[3] = 0;    // Low byte of second 16-bit word (ignored)
        }
        
        // Use non-blocking write if available
        // If not, we need to ensure the write_all is ISR-safe
        self.i2s_tx.write_all(formatted_buffer, formatted_buffer.len() as u32)?;
        
        
        Ok(())
    }
}