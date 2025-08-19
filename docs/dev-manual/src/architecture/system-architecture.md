
# RustyScope System Architecture

This document provides a high-level overview of the software architecture for the RustyScope Telescope Controller. It details the core components, data flow, concurrency model, and key design patterns used throughout the project.

## 1. Guiding Principles

The RustyScope architecture is built on a few key principles:

*   **Modularity & Separation of Concerns:** Each component has a single, well-defined responsibility. The astronomical logic is separate from the motor control, which is separate from the network services.
*   **Asynchronous & Non-Blocking:** The system is designed to be highly responsive. Long-running physical actions (like slewing) do not block the user interface or network API.
*   **Type Safety:** Leveraging Rust's strong type system to represent astronomical and physical concepts (`RightAscension`, `Altitude`, `AxisDrivetrain`), preventing a wide class of bugs at compile time.
*   **Robustness:** The system is designed to handle real-world conditions, including hardware failures (e.g., a missing SD card) and network issues, without crashing.
*   **Testability:** The separation of logic from hardware allows for core components (like astronomical calculations) to be unit-tested on a host machine without needing the physical ESP32 hardware.

## 2. High-Level Component Diagram

The system can be visualized as a set of concurrent tasks communicating through a central, shared state and a command queue.

```mermaid
flowchart TD
    subgraph "External World"
        AlpacaClients[Alpaca Clients<br>(N.I.N.A., KStars, etc.)]
        User[User via On-Device Display]
    end

    subgraph "ESP32 - RustyScope"
        subgraph "Core 0 (Real-Time)"
            F[<b>Motion Timer ISR</b><br>(High-Frequency Pulses)]
        end

        subgraph "Core 1 (Application, System & I/O)"
            A[<b>Alpaca Server Task</b><br>(TCP/UDP Listeners)]
            B[<b>GPS Task</b><br>(UART Reader)]
            C[<b>Time Task</b><br>(LST Calculator)]
            D[<b>Main Task</b><br>(UI Dashboard Loop)]
            E[<b>Commander Task</b><br>(State Machine & Sequencer)]
        end
    end

    SharedState[<b>Arc&lt;Mutex&lt;AppState&gt;&gt;</b><br>The "Digital Twin" of the Mount]
    CommandQueue[<b>FreeRTOS Queue</b><br>(`Command` Enum)]

    AlpacaClients -- HTTP/UDP --> A
    A -- Reads/Writes --> SharedState
    A -- Sends Commands --> CommandQueue

    User -- Interacts with --> D
    D -- Reads --> SharedState

    B -- Updates --> SharedState
    C -- Updates --> SharedState

    CommandQueue -- Received by --> E

    E -- Reads/Writes --> SharedState
    E -- Controls --> G[<b>Motion Planner</b>]

    G -- Feeds Data --> H[<b>I2S Stepper Driver</b>]

    F -- Triggers --> G
```

## 3. Core Components

### 3.1. The Commander Task (`commander_task_entry`)

The **Commander** is the brain of the entire system. It is a classic "Command Processor" or "Actor" that runs in its own FreeRTOS task.

*   **Responsibility:** To serialize and execute all long-running motion and state-change commands.
*   **Mechanism:** It continuously polls a `CommandQueue` for new `Command` enums. It maintains an internal state machine (`CommanderState`) to manage multi-step actions like a two-part slew (`WaitingForCoarseSlew`, `WaitingForFineSlew`) or waiting for a park/home move to complete.
*   **Key Feature:** This architecture prevents race conditions. Since only the Commander can issue motion commands to the `MotionPlanner`, it's impossible for two different sources (e.g., an API call and a UI button press) to move the mount simultaneously in conflicting ways.

### 3.2. The Shared Application State (`app_state.rs`)

The `RustyScopeState` struct, wrapped in an `Arc<Mutex<>>`, serves as the **single source of truth** or "digital twin" of the telescope's state.

*   **Responsibility:** To hold all dynamic data that needs to be shared between tasks.
*   **Contents:**
    *   Current and target celestial coordinates (RA/Dec).
    *   Current local coordinates (Alt/Az).
    *   Mount status flags (`is_slewing`, `is_tracking`, `is_parked`, etc.).
    *   Site information (latitude, longitude, elevation).
    *   Connection status and driver settings.
*   **Data Flow:**
    *   **Written by:** The `CommanderTask` (updating coordinates after telemetry calculations) and API handlers (setting new targets or configurations).
    *   **Read by:** The `MainTask` (for the UI dashboard), the `AlpacaServerTask` (for responding to GET requests), and the `CommanderTask` itself.

### 3.3. The Motion Control Stack

This is a three-layer stack responsible for converting astronomical intent into physical motion.

1.  **Controller (`rustyscope.rs`, `moveslew.rs`, etc.):**
    *   **Role:** The "What to do."
    *   **Logic:** Contains high-level astronomical logic. The `execute_slew_to_target` function converts a target RA/Dec into an absolute motor step position using the current time (LST) and the mount's physical model (`AxisDrivetrain`).

2.  **Planner (`stepper/planner.rs`):**
    *   **Role:** The "How to do it."
    *   **Logic:** A sophisticated motion planner that takes absolute position or continuous rate commands. It implements a trapezoidal acceleration profile for smooth slews and uses a Bresenham's/DDA algorithm to interpolate multi-axis moves. It calculates the precise timing for each motor step.

3.  **Driver (`stepper/i2s0.rs`):**
    *   **Role:** The "Do it."
    *   **Logic:** A lean hardware abstraction layer. It takes buffers of step commands from the Planner and uses the ESP32's I2S peripheral and DMA to generate jitter-free electrical pulses for the stepper motor drivers, with minimal CPU overhead. This is triggered by a high-frequency `esp_timer` ISR.

### 3.4. The Network Stack

1.  **Alpaca Server (`alpacaserver.rs`):**
    *   An `esp-http-server` instance running in its own task (`alpaca_task`).
    *   Uses a **dispatcher pattern** with wildcard URI handlers (`/api/v1/*`) to route all GET and PUT requests to central functions.
    *   These dispatchers parse the requests, validate parameters, and delegate to device-specific handlers (e.g., `handle_telescope_put`).
    *   Crucially, for commands that trigger long actions, the handlers send a `Command` to the `CommandQueue` and return immediately, keeping the API non-blocking.

2.  **Alpaca Discovery (`alpacadiscovery.rs`):**
    *   A simple UDP server listening on port 32227.
    *   It responds to broadcast discovery messages, allowing client software to automatically find the RustyScope device on the network.

### 3.5. Background I/O and Timekeeping Tasks

Several background tasks run continuously to provide the system with essential real-time data.

1.  **GPS Controller (`gps/`):**
    *   A dedicated task that continuously reads NMEA sentences from the GPS module over a UART. It parses this data to provide the system with accurate location (latitude/longitude) and UTC time.

2.  **Time Manager (`time/`):**
    *   A task that depends on the GPS. Once it has an accurate time and location, it continuously calculates the current Local Sidereal Time (LST). LST is absolutely critical for converting celestial coordinates (RA/Dec) to local mount coordinates (Alt/Az) and is the foundation of all pointing calculations.

## 4. Data Flow Example: A "GoTo Slew"

Tracing a `SlewToCoordinates` command demonstrates the entire architecture in action.

1.  **API Request:** An Alpaca client sends a `PUT /api/v1/telescope/0/slewtocoordinatesasync` with RA and Dec data.
2.  **`alpacaserver.rs`:** The `api_puthandler` receives the request. It parses the parameters and calls `handle_telescope_put`.
3.  **`telescope/put.rs`:** The handler converts the raw RA/Dec values into type-safe `RightAscension` and `Declination` objects. It updates the `target_ra` and `target_dec` fields in the `SharedState` and then calls `scope.slew_to_target()`.
4.  **`moveslew.rs`:** `slew_to_target()` creates a `Command::SlewToTarget` enum and pushes it onto the `CommandQueue`. The API call returns `OK` to the client *now*.
5.  **`commander_task_entry`:** In its next loop, the Commander dequeues the `SlewToTarget` command.
    *   It sees its own state is `Idle`.
    *   It calls `execute_slew_to_target()`.
6.  **`moveslew.rs`:** `execute_slew_to_target()`:
    *   Reads the `target_ra` and `target_dec` from `SharedState`.
    *   Gets the current Local Sidereal Time from the `TimeManager`.
    *   Uses the `AxisDrivetrain` model to convert the target coordinates into an **absolute motor step position**.
    *   Calls `planner.move_to_steps()` with this absolute target.
    *   The commander transitions its own state to `CommanderState::WaitingForCoarseSlew`.
7.  **`planner.rs`:** The `MotionPlanner` calculates the required acceleration, cruise, and deceleration phases for the move and enters the `Moving(...)` state.
8.  **`esp_timer` ISR:** The high-frequency timer interrupt repeatedly calls `planner.update()`. In each call, the planner calculates the next step(s) and sends the pulse commands to the `I2S Driver`.
9.  **Completion:**
    *   When the planner reaches the target, `planner.is_slewing()` becomes `false`.
    *   The `commander_task` (in its `WaitingFor...` state) detects this, logs the completion, and initiates the smaller correction slew.
    *   After the correction slew, it commands tracking to begin and finally transitions its own state back to `CommanderState::Idle`, ready for the next command.

## 5. Persistence (`storage/`)

*   The `DeviceConfig` struct defines the schema for all configurable parameters (gear ratios, motor steps, etc.).
*   The `StorageManager` provides a high-level API for saving and loading this struct.
*   It uses `serde` and `serde_json` to serialize the struct into a JSON string.
*   The underlying `SpiffsManager` is a robust, RAII-based wrapper around the ESP-IDF's SPIFFS filesystem driver, handling mounting, formatting, and file I/O. This ensures configuration is preserved across reboots.

## 6. Timekeeping (`time/`) 
