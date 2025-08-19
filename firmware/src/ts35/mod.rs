// src/ts35/mod.rs

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use embedded_graphics::image::Image;
use embedded_graphics::pixelcolor::raw::RawU16;
use esp_idf_svc::hal::delay::{Ets, FreeRtos};
use esp_idf_svc::hal::gpio::{self, PinDriver, Gpio25, Gpio26, Gpio27, Gpio33, Gpio5};
use esp_idf_svc::hal::spi::{
    config::DriverConfig as SpiDriverConfig, config::Config as SpiDeviceConfig, Dma,
    SpiDeviceDriver, SpiDriver,
};
use esp_idf_svc::hal::prelude::*;

// Use the high-level driver crates

use embedded_graphics::mono_font::ascii::FONT_7X13;
use embedded_graphics::mono_font::{MonoTextStyle, MonoFont};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{Dimensions, Point, Primitive, RgbColor, Size};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::{Text, TextStyle};
use embedded_graphics::Drawable;
use esp_idf_svc::sys::{esp_task_wdt_add, esp_task_wdt_reset, xTaskGetCurrentTaskHandle};
use log::{info, warn};
use mipidsi::interface::{SpiInterface};
use mipidsi::models::ST7796;
use mipidsi::options::Orientation;
use mipidsi::Builder;
use profont::{PROFONT_14_POINT, PROFONT_18_POINT};
use tinybmp::Bmp;
use heapless::String as HeaplessString;

//use xpt2046::Xpt2046;

// --- Define the concrete types to make the struct definition readable ---
// This is the full type of the shared SPI bus driver
type SharedSpiDriver<'d> = Arc<SpiDriver<'d>>;
// This is the full type of the display object we will be storing
type MipidsiDisplay<'d> = mipidsi::Display<
    SpiInterface<'d, SpiDeviceDriver<'d, SharedSpiDriver<'d>>, PinDriver<'d, Gpio33, gpio::Output>>,
    ST7796,
    PinDriver<'d, Gpio27, gpio::Output>
>;
// This is the full type of the touch controller
//type TouchController<'d> = Xpt2046<SpiDeviceDriver<'d, SharedSpiDriver<'d>>>;


/// The high-level controller that manages both the display and touch functionality.
pub struct DisplayController<'d> {
    display: MipidsiDisplay<'d>,
    //touch: TouchController<'d>,
    width: u16,
    height: u16,
}

impl<'d> DisplayController<'d> {
    pub fn new(
        // The `new` function takes all the raw peripherals from main.rs
        spi_driver: SpiDriver<'d>,
        lcd_cs: Gpio25,
        touch_cs: Gpio26,
        rst: Gpio27,
        dc: Gpio33,
        backlight: Gpio5,
    ) -> Result<Self, anyhow::Error> {
        
        // --- This is a direct translation of your working code ---
        
        let mut display_enable = PinDriver::output(backlight)?;
        display_enable.set_low()?;

        // 1. Create the sharable, Arc-wrapped SPI Driver
        let shared_spi_driver = Arc::new(spi_driver);

        // 2. Create the virtual device for the DISPLAY
        let display_spi_config = SpiDeviceConfig::new().baudrate(20.MHz().into());
        let display_spi_device = SpiDeviceDriver::new(
            Arc::clone(&shared_spi_driver),
            Some(lcd_cs),
            &display_spi_config,
        )?;

        // 3. Create the virtual device for the TOUCH CONTROLLER
        let touch_spi_config = SpiDeviceConfig::new().baudrate(2.MHz().into());
        let touch_spi_device = SpiDeviceDriver::new(
            Arc::clone(&shared_spi_driver),
            Some(touch_cs),
            &touch_spi_config,
        )?;

        // 4. Initialize the display high-level driver (mipidsi)
        let dc_pin = PinDriver::output(dc)?;
        let mut rst_pin = PinDriver::output(rst)?;


        //let mut buffer = [0u8; 512];
        // Allocate buffer and leak it
        let buffer: &'d mut [u8] = Box::leak(Box::new([0u8; 512]));
        let di = SpiInterface::new(display_spi_device, dc_pin, buffer);
        let mut delay = Ets;
        rst_pin.set_low()?;
        Ets::delay_ms(10u32);
        rst_pin.set_high()?;
        Ets::delay_ms(120u32);

        let width = 320;
        let height = 480;

        let display = Builder::new(ST7796, di)
        .display_size(width, height)
        .orientation(Orientation {
            rotation: mipidsi::options::Rotation::Deg270,
            mirrored: true
        })
        .color_order(mipidsi::options::ColorOrder::Bgr)
        .reset_pin(rst_pin)
        .init(&mut delay)
        .map_err(|e| anyhow::anyhow!("Display init failed: {:?}", e))?;
        
        //let (width, height) = (display.size().width, display.size().height);

        // 5. Initialize the touch high-level driver
        //let touch = Xpt2046::new(touch_spi_device);
        
        info!("Display and Touch controller initialized. Resolution: {}x{}", width, height);

        //Ok(Self { display, touch, width, height })
        Ok(Self { display, width, height })
    }
    // --- High-Level Public API ---
    
    pub fn clear_screen(&mut self) -> Result<(), anyhow::Error> {
        self.display.clear(Rgb565::BLACK).map_err(|e| anyhow::anyhow!("Display clear failed: {:?}", e))
    }

    /// Streams a BMP image from the filesystem to the display without loading it all into RAM.
    /// The image should be a 24-bit, bottom-up BMP.
    pub fn display_splash(&mut self, path: &str) -> Result<(), anyhow::Error> {
        info!("Streaming splash screen from '{}'...", path);

        let mut file = File::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open splash file '{}': {}", path, e))?;

        // 1. Read and parse the BMP header.
        let mut header = [0u8; 54];
        file.read_exact(&mut header)?;
        
        if &header[0..2] != b"BM" { return Err(anyhow::anyhow!("Not a valid BMP file.")); }

        let data_offset = u32::from_le_bytes(header[10..14].try_into()?) as u64;
        let width = i32::from_le_bytes(header[18..22].try_into()?);
        let height = i32::from_le_bytes(header[22..26].try_into()?);
        let bpp = u16::from_le_bytes(header[28..30].try_into()?);

        if bpp != 24 { return Err(anyhow::anyhow!("Only 24-bit BMPs are supported.")); }
        // We expect a 480x320 image now.
        if width != 480 || height != 320 {
             warn!("BMP has unexpected dimensions: {}x{}. Expected 480x320 for landscape mode.", width, height);
        }

        info!("BMP properties: {}x{}, 24bpp, data at offset {}", width, height, data_offset);

        file.seek(SeekFrom::Start(data_offset))?;

        // --- THIS IS THE KEY CODE CHANGE ---
        // 2. The buffer must be large enough for one row of the new image (480 pixels * 3 bytes/pixel).
        let mut buffer = [0u8; 480 * 3]; // 1440 bytes
        
        // 3. The drawing loop now iterates over the correct height (320)
        for y in (0..height).rev() {
            // Read one full row of pixels. `row_byte_size` will be 1440.
            let row_byte_size = (width * 3) as usize;
            file.read_exact(&mut buffer[..row_byte_size])?;

            // The pixel conversion logic is still correct.
            let pixel_iter = buffer[..row_byte_size]
                .chunks_exact(3)
                .map(|bgr| Rgb565::new(bgr[2] >> 3, bgr[1] >> 2, bgr[0] >> 3));
            
            // Draw the line of pixels.
            let line = Rectangle::new(Point::new(0, y), Size::new(width as u32, 1));
            self.display.fill_contiguous(&line, pixel_iter)
                .map_err(|e| anyhow::anyhow!("Failed to draw BMP row at {}: {:?}", y, e))?;
        }

        info!("Splash screen displayed successfully.");
        Ok(())
    }

    /// Streams a 16-bit (RGB565) BMP from the filesystem to the display.
    /// This is a highly efficient method as no color conversion is needed.
    pub fn display_splash_16bit(&mut self, path: &str) -> Result<(), anyhow::Error> {
        info!("Streaming 16-bit BMP splash screen from '{}'...", path);

        // --- NEW: Subscribe this task to the watchdog ---
        // Get a handle to the current task (the `main` task in this case).
        let current_task_handle = unsafe { xTaskGetCurrentTaskHandle() };
        // Add the current task to the watchdog's list. If it's already there, this does nothing.
        unsafe { esp_task_wdt_add(current_task_handle) };

        let mut file = File::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open splash file '{}': {}", path, e))?;

        // 1. Read and parse the BMP header.
        let mut header = [0u8; 54];
        file.read_exact(&mut header)?;
        
        if &header[0..2] != b"BM" { return Err(anyhow::anyhow!("Not a valid BMP file.")); }

        let data_offset = u32::from_le_bytes(header[10..14].try_into()?) as u64;
        let width = i32::from_le_bytes(header[18..22].try_into()?);
        let height = i32::from_le_bytes(header[22..26].try_into()?);
        let bpp = u16::from_le_bytes(header[28..30].try_into()?);

        if bpp != 16 {
            return Err(anyhow::anyhow!("Only 16-bit BMP files are supported. Found {}bpp.", bpp));
        }
        info!("BMP properties: {}x{}, 16bpp, data at offset {}", width, height, data_offset);

        // 2. Seek the file to the beginning of the pixel data.
        file.seek(SeekFrom::Start(data_offset))?;

        // 3. Define the screen area.
        let area = Rectangle::new(Point::zero(), Size::new(width as u32, height as u32));
        
        // 4. Read the file in chunks and draw them.
        // A buffer of 480 pixels * 2 bytes/pixel = 960 bytes.
        let mut buffer = [0u8; 480 * 2];
        
        // BMPs are stored bottom-up.
        for y in (0..height).rev() {
            // Read one full row of pixels (width * 2 bytes).
            let row_byte_size = (width * 2) as usize;
            file.read_exact(&mut buffer[..row_byte_size])?;

            // --- THE KEY OPTIMIZATION ---
            // No color conversion! The data is already in RGB565 format.
            // We just need to interpret each 2-byte chunk as a u16.
            let pixel_iter = buffer[..row_byte_size]
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .map(|raw| Rgb565::from(RawU16::new(raw))); // Create Rgb565 from the raw u16 value

            // Draw this single line of pixels to the screen.
            let line = Rectangle::new(Point::new(0, y), Size::new(width as u32, 1));
            self.display.fill_contiguous(&line, pixel_iter)
                .map_err(|e| anyhow::anyhow!("Failed to draw BMP row at {}: {:?}", y, e))?;
            // --- THE FIX ---
            // After every few rows, reset the watchdog timer.
            // This tells the system "I'm still alive, just busy drawing!"
            // Resetting every 16 rows is a good balance.
            if y % 16 == 0 {
                unsafe { esp_task_wdt_reset() };
            }
        }

        // --- NEW: Unsubscribe from the watchdog (good practice) ---
        // Once the long operation is done, we can remove our task from the watchdog list.
        unsafe { esp_idf_svc::sys::esp_task_wdt_delete(current_task_handle) };

        info!("16-bit splash screen displayed successfully.");
        Ok(())
    }

    pub fn draw_test_pattern(&mut self) -> Result<(), anyhow::Error> {
        Rectangle::new(Point::new(50, 50), Size::new(100, 100))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
            .draw(&mut self.display)
            .map_err(|e| anyhow::anyhow!("Draw failed: {:?}", e))?;
        Ok(())
    }
    /// Draws a line of text, but first clears the background behind it
    /// to prevent ghosting of old text.
    pub fn draw_text_line(
        &mut self,
        text: &str,
        position: Point,
        font: &'static MonoFont,
        text_color: Rgb565,
        background_color: Rgb565,
    ) -> Result<(), anyhow::Error> {
        
        let style = MonoTextStyle::new(font, text_color);
        
        // 1. Create a Text object to measure its size.
        let text_drawable = Text::new(text, position, style);
        
        // 2. Get the bounding box of the text. This is the area we need to clear.
        let bounding_box = text_drawable.bounding_box();

        // 3. Create a style for the background clearing rectangle.
        let background_style = PrimitiveStyleBuilder::new()
            .fill_color(background_color)
            .build();
            
        // 4. Draw the background rectangle to erase the old text.
        bounding_box.into_styled(background_style)
            .draw(&mut self.display)
            .map_err(|e| anyhow::anyhow!("Clear text area failed: {:?}", e))?;

        // 5. Now, draw the new text on top of the cleared area.
        text_drawable.draw(&mut self.display)
            .map_err(|e| anyhow::anyhow!("Draw text failed: {:?}", e))?;
            
        Ok(())
    }

    

    /*
    pub fn get_touch_point(&mut self) -> Option<Point> {
        if let Ok(Some(point)) = self.touch.get_touch() {
            // Map raw coordinates to screen coordinates
            let x = map_range(point.x as u32, 200, 3800, 0, self.width as u32);
            let y = map_range(point.y as u32, 200, 3800, 0, self.height as u32);
            Some(Point::new(x as i32, y as i32))
        } else {
            None
        }
    } */
}

// Helper function for mapping touch coordinates to screen coordinates
fn map_range(x: u32, in_min: u32, in_max: u32, out_min: u32, out_max: u32) -> u32 {
    if x <= in_min { return out_min; }
    if x >= in_max { return out_max; }
    (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
}


// --- NEW: A struct to hold the state of what's ON the screen ---
// We add `PartialEq` so we can easily compare structs.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayState {
    pub header_string: HeaplessString<64>,
    pub axis_x_string: HeaplessString<64>,
    pub axis_y_string: HeaplessString<64>,
    pub axis_z_string: HeaplessString<64>,
    pub status_string: HeaplessString<64>,
    pub utc_string: HeaplessString<64>,
    pub lst_string: HeaplessString<64>,
    pub gps_string: HeaplessString<64>,
    pub lon_string: HeaplessString<64>, // Longitude
    pub lat_string: HeaplessString<64>, // Latitude
    pub webserver_string: HeaplessString<64>,
    pub builtby_string: HeaplessString<64>,
    pub version_string: HeaplessString<64>,
    // Add any other fields you display
}

impl Default for DisplayState {
    fn default() -> Self {
        Self {
            header_string: HeaplessString::new(),
            axis_x_string: HeaplessString::new(),
            axis_y_string: HeaplessString::new(),
            axis_z_string: HeaplessString::new(),
            status_string: HeaplessString::new(),
            utc_string: HeaplessString::new(),
            lst_string: HeaplessString::new(),
            gps_string: HeaplessString::new(),
            lon_string: HeaplessString::new(),
            lat_string: HeaplessString::new(),
            webserver_string: HeaplessString::new(),
            builtby_string: HeaplessString::new(),
            version_string: HeaplessString::new(),
        }
    }
}