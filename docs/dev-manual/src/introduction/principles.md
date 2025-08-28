# Core Design Principles

The RustyScope firmware is built upon a set of core software design principles. Understanding these principles is key to understanding the "why" behind the structure of the code. They are designed to promote safety, maintainability, and real-time performance on a resource-constrained embedded system. This document provides an overview of these principles and explains how they were applied to the RustyScope project.

### 1. The Actor Model for Command Processing

All physical actions that change the state of the mount (slewing, tracking, parking) are handled by a single, dedicated task: the **Commander Task**. This task acts as an "Actor" that receives commands from a message queue.

*   **Why:** This serializes all major operations. It makes it impossible for two conflicting commands (e.g., a `slew` and a `park` from different API clients) to be executed simultaneously, preventing race conditions at the hardware level.
*   **Implementation:** The `commander_task_entry` function loops, receiving `Command` enums from a FreeRTOS queue. It is the *only* part of the system that can issue motion commands to the `MotionPlanner`.

### 2. A Single, Thread-Safe Source of Truth

All dynamic data representing the state of the mount is held in a single struct, `RustyScopeState`, which is shared between all tasks using an `Arc<Mutex<T>>`. This serves as the "digital twin" of the physical hardware.

*   **Why:** This prevents data duplication and ensures consistency. Any task needing to know the current coordinates or slewing status reads from this single, authoritative source. The `Mutex` guarantees that all access is properly synchronized.
*   **Implementation:** The `SharedAppState` type alias (`Arc<Mutex<RustyScopeState>>`) is created once in `main()` and passed to every task that needs it.

### 3. Asynchronous and Non-Blocking by Default

No long-running physical action should ever block a user-facing interface.

*   **Why:** This is critical for a responsive system and is a requirement of the Alpaca standard. A user must be able to query the mount's status even while it is in the middle of a 10-minute slew.
*   **Implementation:** API handlers that trigger actions (e.g., `slew_to_target`) do not perform the action themselves. They simply package the request into a `Command` enum, push it onto the `CommandQueue` for the Commander to handle later, and return an `OK` response immediately.

### 4. Isolate Real-Time from Housekeeping

The ESP32's dual cores are used to create a hard separation between time-critical and non-critical tasks.

*   **Why:** Generating jitter-free motor pulses requires precise, uninterrupted timing. Network activity, filesystem access, or even logging can introduce unpredictable delays (latency spikes) that would disrupt smooth motor control.
*   **Implementation:**
    *   **Core 0 (Real-Time):** The high-frequency `esp_timer` interrupt that drives the `MotionPlanner` is pinned to Core 0.
    *   **Core 1 (System/I/O):** The Alpaca web server, GPS polling task, and other "housekeeping" tasks are pinned to Core 1, ensuring they can never preempt the critical motion control timer.

### 5. Type-Safe Physical and Astronomical Modeling

Rust's powerful type system is used to represent real-world physical and astronomical concepts, making invalid states unrepresentable in code.

*   **Why:** This prevents an entire class of logical errors. Instead of passing around `f64` values for angles, we use specific types that enforce their own rules.
*   **Implementation:**
    *   `Declination`: A struct that cannot be constructed with a value outside the `[-90, 90]` degree range.
    *   `RightAscension`: A struct that automatically wraps its value around the 24-hour clock.
    *   `AxisDrivetrain`: A struct that encapsulates the physical gear ratios and motor steps, providing a single, authoritative place for all mechanical-to-angular conversions.

### 6. Layered Abstraction in the Motion Stack

The motion control system is a clear three-layer stack, with each layer having a distinct responsibility.

*   **Why:** This is a classic example of separation of concerns, making the complex process of motion control easier to understand, test, and maintain.
*   **Implementation:**
    *   **Controller Layer (`moveslew.rs`):** The "What to do." It translates astronomical targets (RA/Dec) into physical targets (absolute motor steps).
    *   **Planner Layer (`planner.rs`):** The "How to do it." It takes a target step position and generates a smooth, trapezoidally-accelerated motion profile.
    *   **Driver Layer (`i2s0.rs`):** The "Do it." It takes commands from the planner and generates the raw electrical pulses for the motors using I2S and DMA.

### 7. Graceful Degradation

The system is designed to handle the failure of non-critical components without crashing the entire application.

*   **Why:** In a real-world field deployment, peripherals can fail. The core pointing and tracking functionality should continue to work if possible.
*   **Implementation:** The `SystemServices` initializer can handle a failure to mount the SD card. In this case, it simply logs a warning and does not start the web server (which depends on the SD card for its files), but the rest of the system (motion control, GPS, etc.) continues to boot and operate.