
# GPS Controller (`gpscontroller.rs`)

The GPS Controller is a fundamental system service responsible for providing accurate time, location, and elevation data to the rest of the RustyScope application. It is a self-contained, concurrent module that runs independently, continuously parsing data from a hardware GPS unit.

## 1. Core Responsibilities

*   **Hardware Interfacing:** Manages the UART communication channel connected to the physical GPS module.
*   **NMEA Protocol Parsing:** Decodes standard NMEA 0183 sentences (like `$GPGGA`, `$GPRMC`, `$GPZDA`) to extract meaningful data.
*   **State Management:** Maintains the latest-known GPS data (position, fix status, satellite count, etc.) in a thread-safe manner.
*   **System Time Synchronization:** Provides a method to set the ESP32's main system clock (RTC) using the highly accurate time provided by the GPS satellites.

## 2. Architecture: A Concurrent Data Provider

The `GpsController` is designed as a background service that runs in its own dedicated FreeRTOS task. This architectural choice is critical for a responsive system.

```mermaid
flowchart TD
    subgraph "External World"
        GPS_HW[GPS Hardware Module]
    end

    subgraph "ESP32 - RustyScope"
        subgraph "GpsController Module"
            A[<b>GpsController Struct</b><br>Public API (`get_data`, `set_system_time`)]
            B[<b>GPS Task (`gps_task`)</b><br>Runs in the background]
            C[<b>Shared GpsData</b><br>Arc&lt;Mutex&lt;GpsData&gt;&gt;]
            D[UART Driver]
        end

        subgraph "Other System Services"
            TimeManager[Time Manager]
            RustyScopeController[RustyScope Controller]
        end
    end

    GPS_HW -- NMEA Sentences (Serial) --> D
    D -- read by --> B
    B -- parses & updates --> C
    A -- provides public access to --> C

    TimeManager -- reads data from --> A
    RustyScopeController -- reads data from --> A
```

### Key Components:

1.  **`GpsController` Struct:** This is the public-facing object. Other parts of the system hold an `Arc<GpsController>` to interact with it. It exposes a clean API and hides the complexity of the background task.
2.  **`GpsData` Struct:** A simple `struct` that acts as a container for all parsed GPS information. This includes latitude, longitude, elevation, fix status, satellite count, and the crucial `epoch_utc`.
3.  **Shared State (`Arc<Mutex<GpsData>>`):** The `GpsController` holds the single, authoritative copy of the `GpsData` inside a `Mutex` to ensure thread-safe access.
4.  **`gps_task` (Background Thread):** This is the workhorse.
    *   It is spawned by `GpsController::new()` and runs for the lifetime of the application.
    *   It owns the `UartDriver`.
    *   It continuously reads raw byte data from the UART into a buffer.
    *   It intelligently handles partial NMEA sentences, accumulating data until a complete line (`\n`) is received.
    *   It calls the `parse_nmea_sentence` function to decode the data.
    *   It updates the shared `GpsData` struct, taking a lock only for the brief moment needed to copy the new data.

## 3. Data Flow: From Satellite to System

1.  **Reception:** The `gps_task` blocks on `uart.read()`, waiting for data from the GPS module.
2.  **Assembly:** Raw bytes are appended to a `partial_sentence` string buffer. The task loops, looking for newline characters.
3.  **Parsing (`parse_nmea_sentence`):**
    *   When a full sentence is found, it's passed to the parser.
    *   The parser checks the sentence ID (e.g., `$GPGGA`).
    *   Based on the ID, it splits the sentence by commas and extracts the relevant fields.
    *   **`$GPGGA`:** Provides high-quality position, fix status (`fix_quality > 0`), satellite count, and elevation.
    *   **`$GPRMC`:** Provides a secondary source for position and fix status, which is useful for redundancy.
    *   **`$GPZDA`:** This is the most important sentence for time. It provides the full date (day, month, year) and time, which is used to construct a high-precision `hifitime::Epoch`.
4.  **State Update:** The parsed data is written into a local `GpsData` struct within the `gps_task`. After a sentence is fully processed, the task acquires a lock on the shared `GpsData` and replaces its contents with the new data. This "update-then-lock-and-copy" pattern is highly efficient and minimizes the time the mutex is held.
5.  **Consumption:**
    *   The `TimeManager` periodically calls `gps_controller.get_data()` to retrieve the longitude for its LST calculations.
    *   During startup, `SystemServices` calls `gps_controller.set_system_time()`. This method reads the `epoch_utc` from the shared `GpsData`, converts it to a `timeval` struct, and makes an `unsafe` FFI call to the `settimeofday` system function.

## 4. Key Design Decisions

*   **Decoupling:** The `GpsController` knows nothing about the `TimeManager` or `RustyScope`. It simply provides data. This makes the component highly reusable and easy to test in isolation.
*   **Efficiency:** The background task is optimized to minimize lock contention. It performs all heavy lifting (reading, parsing) without holding the mutex, only acquiring it for a near-instantaneous copy operation.
*   **Robustness:** The NMEA parser is designed to handle malformed or incomplete sentences gracefully, simply ignoring them and waiting for the next valid one. It uses `unwrap_or(default)` to avoid panicking if a field is empty or invalid, ensuring the system remains stable even with noisy GPS data.
*   **Thread Safety:** The `GpsController` uses an `Arc<Mutex<GpsData>>` to ensure thread safety. The `Mutex` is only held for a brief moment during the update, ensuring minimal impact on the background task.