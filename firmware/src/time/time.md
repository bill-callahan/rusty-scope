
# Time Manager (`time/time.rs`)

The Time Manager is the clockwork heart of the RustyScope system. Its primary responsibility is to provide all other components with accurate, continuously updated time information, most critically the **Local Sidereal Time (LST)**, which is the foundation of all equatorial coordinate calculations.

## 1. Core Responsibilities

*   **Maintain System Time:** It keeps track of the current UTC time, relying on the `GpsController` to provide an initial, highly accurate synchronization source.
*   **Calculate Local Sidereal Time (LST):** Its main purpose is to perform the periodic, computationally intensive conversion from Universal Time (UTC) to Local Sidereal Time (LST).
*   **Provide a Centralized Time Source:** It acts as the single source of truth for time-related data, ensuring all components are working from the same clock.

## 2. Architecture: A Concurrent Timekeeper

Similar to the `GpsController`, the `TimeManager` is designed as a background service that runs in its own dedicated FreeRTOS task. This ensures that the LST calculation, which must be run frequently, does not interfere with other system operations.

```mermaid
flowchart TD
    subgraph "Other System Services"
        GPS[<b>GpsController</b>]
        Commander[<b>Commander Task</b>]
        API[<b>Alpaca Server</b>]
        UI[<b>Main Task (UI)</b>]
    end

    subgraph "ESP32 - RustyScope"
        subgraph "TimeManager Module"
            A[<b>TimeManager Struct</b><br>Public API (`get_state`, `get_lst`)]
            B[<b>Time Task (`sidereal_task`)</b><br>Runs in the background]
            C[<b>Shared TimeState</b><br>Arc&lt;Mutex&lt;TimeState&gt;&gt;]
        end
    end

    GPS -- provides Longitude --> B
    B -- `Epoch::now()` --> D{System Clock (RTC)}
    B -- calculates & updates --> C
    A -- provides public access to --> C

    Commander -- reads LST from --> A
    API -- reads LST/UTC from --> A
    UI -- reads LST/UTC from --> A
```

### Key Components:

1.  **`TimeManager` Struct:** The public-facing object, shared via an `Arc<TimeManager>`. It provides a clean API for other modules to get the current time state.
2.  **`TimeState` Struct:** A container for all relevant time data, including the calculated `local_sidereal_time`, the current `utc_epoch`, and the observer's `site_longitude`.
3.  **Shared State (`Arc<Mutex<TimeState>>`):** The single, authoritative copy of the `TimeState`, protected by a `Mutex` for thread-safe access.
4.  **`sidereal_task` (Background Thread):**
    *   Spawned by `TimeManager::new()` and runs for the lifetime of the application.
    *   **Initialization:** It first waits in a loop until it can acquire a valid longitude from the `GpsController`. This is a critical step, as LST is location-dependent.
    *   **Main Loop:** It runs a periodic loop (typically once per second). In each iteration, it calls its internal `update()` method.

## 3. Data Flow and LST Calculation

1.  **Dependency:** The `TimeManager::new()` function takes an `Arc<GpsController>` as an argument, establishing its dependency on the GPS service.
2.  **Initialization:** The `sidereal_task` starts and immediately begins polling the `GpsController` for a valid longitude. Once acquired, it stores this longitude in its own `TimeState`.
3.  **Update Tick (`update()`):** The core of the `TimeManager`'s work happens here.
    *   The task calls `hifitime::Epoch::now()`. This reads the current time from the ESP32's system clock, which was previously set by the `GpsController`.
    *   It then calls the static `compute_lmst` function.
4.  **LST Computation (`compute_lmst`):**
    *   This function implements the standard algorithm for calculating LST from UTC.
    *   It gets the current Julian Date from the `Epoch`.
    *   It calculates the Greenwich Mean Sidereal Time (GMST) based on the Julian Date.
    *   It applies the observer's longitude (`LST = GMST + Longitude`) to get the final Local Sidereal Time.
    *   The result is normalized to the range `[0, 24)` hours.
5.  **State Update:** The newly calculated LST and the current `Epoch` are stored in the shared `TimeState` struct, protected by a brief mutex lock.
6.  **Consumption:**
    *   The **`CommanderTask`**, when executing a slew, calls `time_manager.get_lst()` to get the current LST. This is essential for the `RA -> Hour Angle -> Motor Steps` conversion.
    *   The **`Telemetry`** update logic does the same when converting the current motor steps back into RA/Dec coordinates.
    *   The **Alpaca API** and **UI Dashboard** call getters to display the current UTC and LST to the user.

## 4. Key Design Decisions

*   **Dedicated Task:** Separating the LST calculation into its own task is crucial for performance. It ensures a fresh LST value is always available to the motion control system without requiring the motion controller to perform the calculation itself, which could introduce timing jitter.
*   **Dependency Injection:** By passing the `GpsController` into the `TimeManager`'s constructor, the dependency is made explicit and clear. This follows best practices for creating loosely-coupled components.
*   **High-Precision Library:** The use of the `hifitime` crate provides access to robust, well-tested astronomical time scales and calculations (like Julian Date conversion), which is far superior to attempting to implement this complex logic from scratch.
*   **Thread-Safe Data:** The `TimeState` struct is protected by a `Mutex` to ensure thread-safe access to the shared state. This is essential for multi-threaded applications like the ESP32, where multiple tasks may need to access the time state concurrently.