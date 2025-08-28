# GPS: The Location and Time Source

The `GpsController` is a fundamental I/O service in RustyScope. It provides two of the most critical pieces of information for astronomical calculations: the precise location of the telescope on Earth (latitude and longitude) and the precise current time. Accurate time and location are the bedrock upon which all other pointing and tracking calculations are built.

## The `GpsController`

The `GpsController`, defined in `src/gps/gpscontroller.rs`, is responsible for managing the GPS module. It runs as a dedicated background task with two primary responsibilities:

1.  **NMEA Data Parsing:** It continuously reads data from the GPS module over a UART connection. This data arrives in the form of NMEA sentences (e.g., `$GPGGA`, `$GPRMC`), which are standardized text-based messages. The controller includes a parser that decodes these sentences to extract key information like latitude, longitude, elevation, the number of visible satellites, and the current UTC time.

2.  **State Management:** The parsed information is stored in a `GpsData` struct. This struct is protected by a `Mutex` and wrapped in an `Arc`, allowing its data to be safely accessed by any other task in the system that needs it.

```rust
// Simplified GpsData struct
pub struct GpsData {
    pub has_fix: bool,
    pub satellites_used: u32,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: f32,
    pub epoch_utc: Option<Epoch>,
}
```

By running the UART reading and parsing in a dedicated background task, the `GpsController` ensures that the main application logic is never blocked by I/O operations.

## PPS: High-Precision Time Sync

While the NMEA sentences provide the time, they are not perfectly synchronized. The time they report can be delayed by several hundred milliseconds due to the time it takes to transmit the data over the slow UART connection. For high-precision tracking, this is not good enough.

To solve this, RustyScope uses the **Pulse-Per-Second (PPS)** signal from the GPS module. This is a separate electrical signal on a dedicated wire that sends out a single, very sharp pulse precisely at the beginning of each new second, as defined by the atomic clocks in the GPS satellites. This pulse is accurate to within nanoseconds.

The `pps_sync.rs` module is responsible for handling this signal.

1.  **GPIO Interrupt:** It configures a GPIO pin as an input and sets up an interrupt to trigger on the rising edge of the PPS signal.

2.  **The Interrupt Service Routine (ISR):** When the PPS pulse arrives, the ISR is executed. This ISR has one, critical job: it gets the current timestamp from the NMEA data (which is close, but not perfect) and then **sets the ESP32's internal system clock** to be precisely aligned with that second.

This process, known as "disciplining the clock," effectively turns the ESP32's internal crystal oscillator into an atomic clock. It corrects for any drift in the local oscillator caused by temperature changes or manufacturing imperfections. This ultra-precise timekeeping is essential for the `TimeManager` to calculate the Local Sidereal Time (LST) accurately, which in turn is critical for the mount's tracking and pointing calculations.
