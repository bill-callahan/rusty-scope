
# System Services (`systemservices.rs`)

The `SystemServices` module is the master orchestrator and service locator for the entire RustyScope application. Its primary role is to initialize all hardware peripherals and high-level software services in the correct order during system startup. It then acts as a central, organized container for these services, providing the main `RustyScope` controller with all the tools it needs to operate.

## 1. Core Responsibilities

*   **Initialization Orchestration:** To perform the complex, ordered sequence of initializations required to bring the system online. This includes core system services, networking, storage, and all hardware peripherals.
*   **Service Location:** To hold thread-safe handles (`Arc<T>`) to all major system services, acting as a single, injectable dependency for the main application logic.
*   **Hardware Abstraction:** To configure and create the drivers for all physical hardware (I2S, SPI, UART, GPIO), abstracting the low-level hardware details away from the main application.
*   **Startup Synchronization:** To block execution until critical subsystems (like GPS and Time) are ready, ensuring the rest of the application starts from a known, valid state.

## 2. Architecture: A Sequential Bring-up and Service Hub

The `SystemServices::new()` function is the entry point for system initialization. It executes a carefully ordered sequence of steps, with each step building upon the previous one. Once initialization is complete, the `SystemServices` struct itself becomes a read-only "service hub" passed to other components.

```mermaid
flowchart TD
    subgraph "main()"
        A["main() calls `SystemServices::new()`"]
    end

    subgraph "SystemServices::new() Execution Flow"
        B(<b>Start</b>: Take Peripherals) -->
        C(Initialize NVS & Event Loop) -->
        D(<b>WiFi Initialization</b><br>`wifi_create()`) -->
        E(<b>Storage Initialization</b><br>SPIFFS & SD Card) -->
        F(<b>Web Server Initialization</b><br>`create_webserver()`<br><i>Depends on WiFi & SD Card</i>) -->
        G(<b>GPS Controller Initialization</b><br>`GpsController::new()`<br>Spawns `gps_task`) -->
        H(<b>Time Manager Initialization</b><br>`TimeManager::new()`<br><i>Depends on GPS</i><br>Spawns `sidereal_task`) -->
        I(<b>Wait for Critical Sync</b><br>Loop until GPS has fix<br>Loop until LST is valid) -->
        J(<b>Motion System Initialization</b><br>I2S Driver, Motion Planner<br>High-res `esp_timer` ISR) -->
        K(<b>Display Controller Initialization</b><br>`DisplayController::new()`) -->
        L(<b>Return `Ok(Self)`)</b>
    end

    subgraph "Resulting SystemServices Struct"
        M["<b>SystemServices {</b><br>  planner: Arc&lt;Mutex&lt;...&gt;&gt;,<br>  gps: Arc&lt;GpsController&gt;,<br>  time: Arc&lt;TimeManager&gt;,<br>  display: Arc&lt;Mutex&lt;...&gt;&gt;,<br>  spiffs: StorageManager,<br>  _wifi: Box&lt;EspWifi&gt;,<br>  ...<br><b>}</b>"]
    end

    A --> B
    L --> M
    M -- passed to --> N["`RustyScope::new()`"]
```

## 3. The Initialization Sequence in Detail

The `SystemServices::new()` function proceeds in a specific, dependency-aware order:

1.  **Core System Handles:** It takes ownership of the foundational ESP-IDF resources: `Peripherals`, `EspSystemEventLoop`, and `EspDefaultNvsPartition`. This is always the first step.
2.  **Networking (WiFi):** It initializes the WiFi stack and connects to the configured network. This must happen early, as it provides the IP address needed by later services like the web server and discovery.
3.  **Storage (SPIFFS & SD Card):** It initializes the non-volatile storage filesystems. This is a robust process that can automatically format SPIFFS if needed and can gracefully handle the absence of an SD card.
4.  **Web Server:** The HTTP server is initialized *after* WiFi (to get an IP) and the SD card (to serve static files like the splash screen image or setup pages). If the SD card fails to mount, the web server is not started, demonstrating graceful degradation.
5.  **GPS Controller:** The UART for the GPS is configured, and the `GpsController` is created. This spawns the `gps_task` which begins parsing NMEA data in the background.
6.  **Time Manager:** The `TimeManager` is created, taking a handle to the `GpsController` as a dependency. This spawns the `sidereal_task`, which waits for the `gps_task` to acquire a location before it starts its LST calculations.
7.  **Critical Synchronization (Blocking):** This is a key design choice for robustness. The `new()` function enters a blocking loop, deliberately pausing the entire application startup until:
    *   The `gps_task` reports a valid fix and the system time has been set.
    *   The `sidereal_task` reports its first valid LST calculation.
    This ensures that when the main `RustyScope` controller is created, it starts with accurate time and location, dramatically simplifying its own initialization logic.
8.  **Motion System:**
    *   The `I2sStepperDriver` is initialized.
    *   The `MotionPlanner` is created.
    *   A high-resolution `esp_timer` is configured to call `planner.update()` as its ISR callback. This is a complex step involving `unsafe` code to pass a pointer to the planner into the C-style ISR context.
9.  **Display Controller:** The final peripheral, the display, is initialized, and the splash screen is shown to the user.
10. **Completion:** The fully populated `SystemServices` struct is returned to `main()`.

## 4. Key Design Decisions

*   **Centralized Initialization:** Concentrating all hardware and service initialization in one place makes the system's startup process clear, understandable, and easy to debug. It avoids scattering initialization logic across multiple files.
*   **Dependency Injection:** The `SystemServices` struct acts as a service locator that is "injected" into the `RustyScope` constructor. This is a clean pattern that makes dependencies explicit and avoids global state.
*   **Thread-Safe Handles:** Every service held in the `SystemServices` struct is wrapped in a thread-safe handle (`Arc<T>` or `Arc<Mutex<T>>`). This is what allows the various tasks (Commander, Alpaca, Main) to safely share access to these services.
*   **RAII for Peripherals:** Many of the drivers created (like `SpiffsManager` and others from `esp-idf-svc`) follow the RAII pattern. The `SystemServices` struct takes ownership of these drivers, and they are automatically de-initialized and their resources freed if `SystemServices` is ever dropped.
*   **`SendWrapper` for Unsafe Concurrency:** For types provided by `esp-idf-svc` that are not inherently `Send` (like `EspHttpServer`), a `SendWrapper` is used. This is a necessary `unsafe` construct that signals to the Rust compiler a developer's guarantee that the non-`Send` type will be handled in a thread-safe manner, enabling it to be stored in the multi-threaded `SystemServices` context.
*   **Graceful Degradation:** The system is designed to gracefully degrade if any of the subsystems fail. For example, if the SD card fails to mount, the web server is not started. This is a key feature for a system that needs to be reliable in the field.
*   **Critical Synchronization:** The `new()` function enters a blocking loop, deliberately pausing the entire application startup until:
    *   The `gps_task` reports a valid fix and the system time has been set.
    *   The `sidereal_task` reports its first valid LST calculation.
    This blocking initialization ensures that when the main `RustyScope` controller is created, it starts with accurate time and location, which dramatically simplifies its own logic.
