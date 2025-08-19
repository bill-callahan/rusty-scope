// src/storage/sdcard.rs

use esp_idf_svc::hal::gpio::{AnyIOPin, InputPin, OutputPin};
use esp_idf_svc::hal::peripheral::Peripheral;
use esp_idf_svc::hal::sd::{spi::SdSpiHostDriver, SdCardConfiguration, SdCardDriver};
use esp_idf_svc::hal::spi::{config::DriverConfig, Dma, SpiDriver};
use esp_idf_svc::fs::fatfs::{Fatfs, MountedFatfs};
use log::info;
use std::fs;
use std::io::Write;

// --- Type aliases to manage the extreme complexity of the final type ---
// This is the full, concrete type of the SD card driver stack.
type FullSdCardDriver<'d> = SdCardDriver<'d, SdSpiHostDriver<'d>>;
// This is the full, concrete type of the mounted filesystem object.
type FullMountedFatfs<'d> = MountedFatfs<'d, Fatfs<FullSdCardDriver<'d>>>;

/// A manager for the SD Card filesystem. Its existence keeps the filesystem mounted.
pub struct SdCardManager<'d> {
    mount_path: String,
    // We store the final RAII guard object. The type alias makes this readable.
    _mounted_fatfs: FullMountedFatfs<'d>,
}

impl<'d> SdCardManager<'d> {
    pub fn new<SPI, MOSI, MISO, SCLK, CS>(
        spi: impl Peripheral<P = SPI> + 'd,
        mosi: impl Peripheral<P = MOSI> + 'd,
        miso: impl Peripheral<P = MISO> + 'd,
        sclk: impl Peripheral<P = SCLK> + 'd,
        cs: impl Peripheral<P = CS> + 'd,
        mount_path: &str,
    ) -> Result<Self, anyhow::Error>
    where
        SPI: esp_idf_svc::hal::spi::Spi,
        MOSI: OutputPin,
        MISO: InputPin,
        SCLK: OutputPin,
        CS: OutputPin,
    {
        info!("Initializing SD card via SPI...");

        // This is a direct, line-by-line implementation of the example you provided.
        let spi_driver = SpiDriver::new(
            spi, sclk, mosi, Some(miso), &DriverConfig::default().dma(Dma::Auto(4096)),
        )?;

        let sd_card_driver = SdCardDriver::new_spi(
            SdSpiHostDriver::new(
                spi_driver,
                Some(cs),
                AnyIOPin::none(), // CD pin
                AnyIOPin::none(), // WP pin
                AnyIOPin::none(), // Power pin
                None,             // For newer ESP-IDF versions
            )?,
            &SdCardConfiguration::new(),
        )?;

        // The example shows these two steps combined, which is the key.
        // 1. `Fatfs::new_sdcard` creates the filesystem object.
        // 2. `MountedFatfs::mount` takes that object and mounts it.
        let mounted_fatfs = MountedFatfs::mount(
            Fatfs::new_sdcard(0, sd_card_driver)?,
            mount_path,
            4 // Max open files
        )?;
        
        info!("SD card FAT filesystem mounted successfully at '{}'", mount_path);

        Ok(Self {
            mount_path: mount_path.to_string(),
            _mounted_fatfs: mounted_fatfs,
        })
    }

    pub fn mount_path(&self) -> &str {
        &self.mount_path
    }
    
    pub fn read_file_string(&self, file_name: &str) -> Result<String, std::io::Error> {
        let full_path = format!("{}/{}", self.mount_path, file_name);
        fs::read_to_string(full_path)
    }

    pub fn write_file_string(&self, file_name: &str, data: &str) -> Result<(), std::io::Error> {
        let full_path = format!("{}/{}", self.mount_path, file_name);
        let mut file = fs::File::create(full_path)?;
        file.write_all(data.as_bytes())
    }
}