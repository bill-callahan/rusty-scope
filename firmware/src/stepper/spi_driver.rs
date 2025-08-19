// src/stepper/spi_driver.rs

use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{AnyIOPin, Output, PinDriver};
use esp_idf_svc::hal::spi::{ Dma, SpiDriver, SpiDriverConfig, SPI3};
use esp_idf_svc::hal::spi::config::{BitOrder, Config};
use esp_idf_svc::hal::{spi, prelude::*};
use esp_idf_svc::sys::EspError;

pub struct SpiStepperDriver<'d> {
    // We will use a SpiDeviceDriver, which represents a single device on the bus.
    device: spi::SpiDeviceDriver<'d, spi::SpiDriver<'d>>,
    latch: PinDriver<'d, AnyIOPin, Output>,
}

impl<'d> SpiStepperDriver<'d> {
    pub fn new(
        spi_peripheral: SPI3,
        sclk: impl Into<AnyIOPin>,
        mosi: impl Into<AnyIOPin>,
        latch: impl Into<AnyIOPin>,
        baudrate: Hertz,
    ) -> Result<Self, EspError> {
        
        // 1. Create the SPI bus driver.
        let driver = SpiDriver::new(
            spi_peripheral,
            sclk.into(),
            mosi.into(),
            Option::<AnyIOPin>::None, // No MISO
            // We use a simple driver config, disabling DMA for ISR safety.
            &SpiDriverConfig::new().dma(Dma::Disabled),
        )?;

        // 2. Create the configuration for our specific device (the shift register).
        let config = Config::new()
            .baudrate(baudrate)
            .bit_order(BitOrder::LsbFirst);

        // 3. Create a "virtual" device driver from the bus driver.
        //    We pass `None` for CS because we are controlling the latch pin manually.
        let device = spi::SpiDeviceDriver::new(driver, Option::<AnyIOPin>::None, &config)?;
        
        let mut latch_driver = PinDriver::output(latch.into())?;
        latch_driver.set_low()?;
        
        Ok(Self {
            device,
            latch: latch_driver,
        })
    }
    
    #[link_section = ".iram1.text"]
    #[inline(never)]
    pub fn send_steps(&mut self, buffer: &[u8]) -> Result<(), EspError> {
        // Your logic is perfect, just needs to be adapted to the SpiDeviceDriver API

        
        
        // The `write` method on the device driver handles the transaction.
        self.device.write(buffer)?;
        
        // Pulse the latch pin to update outputs.
        self.latch.set_high()?;
        Ets::delay_us(10); // Minimal delay for latch.
        self.latch.set_low()?;
        
        Ok(())
    }
}