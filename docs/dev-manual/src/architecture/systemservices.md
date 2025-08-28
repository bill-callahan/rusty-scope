# The `main` Function and System Bring-up

The `main` function in `src/main.rs` is the entry point for the RustyScope firmware. It orchestrates the entire system initialization process, from setting up low-level hardware services to launching the high-level application tasks.

## System Initialization with `SystemServices`

The first and most critical step is the creation of the `SystemServices` struct. This struct acts as a centralized container for all core hardware and system-level services.

```rust
// In main.rs
let system = Box::leak(Box::new(SystemServices::new()?));
```

The `SystemServices::new()` function is a comprehensive setup routine that performs the following initializations:

*   **Core Dependencies:** Takes ownership of the ESP32 peripherals, the system event loop, and the non-volatile storage (NVS) partition.
*   **Networking:** Initializes the Wi-Fi driver, connects to the configured network, and obtains an IP address.
*   **Storage:** Initializes the SPIFFS filesystem for persistent on-device configuration storage and mounts the SD card if present.
*   **GPS and Time:** Initializes the GPS module and its UART. It then starts the `TimeManager`, a background task that continuously synchronizes the system clock with the highly accurate time signals from the GPS, including the Pulse-Per-Second (PPS) signal for atomic-level precision.
*   **Motion Control:** This is the heart of the real-time system. It sets up the chosen stepper driver (e.g., `BitbangStepperDriver`), creates the `MotionPlanner`, and configures a high-resolution hardware timer to trigger the motion control Interrupt Service Routine (ISR) at a precise frequency (e.g., 5 kHz).
*   **Display:** If the `display` feature is enabled, it initializes the display controller and shows a splash screen.

### The `Box::leak()` Pattern

The `SystemServices` instance is wrapped in a `Box` to allocate it on the heap, giving it a stable memory address. `Box::leak()` is then used to create a `'static` reference to it. This is a deliberate design choice that allows this central struct to be safely shared with the long-running FreeRTOS tasks without complex lifetime management.

## Creating the Application State and Controller

With the system services online, `main` then constructs the high-level application components:

```rust
// In main.rs
let app_state  = Arc::new(Mutex::new(RustyScopeState::default()));
let rustyscope: Arc<RustyScope<'static>> = Arc::new(RustyScope::new(system, Arc::clone(&app_state))?);
```

*   **`RustyScopeState`**: This struct is the single source of truth for the mount's dynamic state (e.g., current coordinates, tracking status, target coordinates). It is wrapped in an `Arc<Mutex<>>` to provide safe, shared access from any task.
*   **`RustyScope`**: This is the primary controller object. It encapsulates the core astronomical logic (GoTo, Sync, Tracking) and holds references to the `SystemServices` and the shared `RustyScopeState`.

## Launching the Main Tasks

Next, `main` spawns the two primary application tasks onto the ESP32's second core (Core 1), leaving Core 0 free for the real-time motion ISR.

```rust
// In main.rs
// Create and pin the Alpaca server task to Core 1
xTaskCreatePinnedToCore(...);

// Create and pin the Commander task to Core 1
xTaskCreatePinnedToCore(...);
```

*   **Alpaca Server Task:** Runs the web server that implements the ASCOM Alpaca API. It handles HTTP requests from astronomical software and translates them into commands.
*   **Commander Task:** This is the central nervous system of the application. It runs a state machine that receives commands from the Alpaca task and orchestrates the step-by-step execution of complex operations like slewing, parking, and homing.

## The Main Task Loop

After launching the other tasks, the `main` function itself enters an infinite loop. Its role depends on the `display` compile-time feature:

*   **With Display:** The main loop becomes the UI thread, responsible for rendering the dashboard. It periodically fetches data from the `RustyScopeState` and other services, formats it, and draws it to the screen. It uses a "diffing" engine to only redraw parts of the screen that have changed, optimizing performance.
*   **Without Display (Headless Mode):** The loop has minimal responsibility. It simply keeps the main task alive and periodically logs system health metrics (like free heap and stack usage) to the console.

This architecture ensures that the user interface or logging operations on the main task can never interfere with the critical command processing or real-time motion control happening in the other tasks and cores.