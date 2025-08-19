# Project Goals

This document outlines the primary technical and philosophical goals of the RustyScope project from a developer's perspective.

### 1. Create a Reference Architecture for Embedded Rust

The foremost goal is to serve as a high-quality, real-world example of a complex embedded systems project built in Rust. This involves demonstrating solutions to common embedded challenges:
*   **Concurrency:** Safely manage multiple independent tasks (network server, motor control, sensor polling) using Rust's ownership model and concurrency primitives like `Arc`, `Mutex`, and FreeRTOS message queues.
*   **Real-Time Control:** Interface with hardware timers and peripherals (I2S, DMA) to generate precise, jitter-free signals for motor control, a critical requirement for astrophotography.
*   **Hardware Abstraction:** Build safe, idiomatic Rust wrappers around C-based vendor APIs (ESP-IDF) and hardware registers, demonstrating a clean separation between low-level drivers and high-level application logic.
*   **State Management:** Design a robust, thread-safe "single source of truth" for the system's state (the `AppState` struct) that can be safely accessed and modified by multiple concurrent tasks.
*   **Error Handling & Resilience:** Utilize Rust's `Result` and `?` operator to create a system that can gracefully handle failures (e.g., sensor disconnects, network issues) without crashing, and provide clear log output for debugging.

### 2. Achieve High-Precision, Jitter-Free Motion

The core function of a telescope mount is smooth, precise motion. A key technical goal is to leverage the unique capabilities of the ESP32 to achieve this:
*   **I2S for Stepping:** Use the I2S peripheral, normally for audio, to drive stepper motors via DMA. This offloads the CPU entirely from the task of generating step pulses, resulting in a perfectly stable, jitter-free pulse train that is impossible to achieve with software-based delays.
*   **Advanced Motion Planning:** Implement a trapezoidal acceleration planner to ensure smooth starts and stops, protecting the mount's mechanical components from unnecessary stress and vibration.

### 3. Implement a Modern, Standards-Based Network API

To be a useful tool, the controller must integrate seamlessly with existing astronomy software.
*   **Full Alpaca Compliance:** Provide a complete, standards-compliant implementation of the ASCOM Alpaca protocol. This ensures out-of-the-box compatibility with a wide range of planetarium and astrophotography software (N.I.N.A., KStars, etc.) on any platform.
*   **Asynchronous & Non-Blocking:** Build the API server on the `esp-idf-svc` asynchronous web server. All long-running commands (like slewing) are handled via a command queue, ensuring the API remains responsive at all times, as required by the Alpaca standard.
*   **Type-Safe by Design:** Use the `serde` crate to define Rust structs that map directly to the Alpaca JSON API. This eliminates an entire class of bugs related to manual JSON string construction and ensures all API responses are well-formed.

### 4. Prioritize Developer Experience and Maintainability

The project should be as easy to understand and modify as it is to use.
*   **Clear Separation of Concerns:** The codebase is intentionally divided into distinct, logical modules: `motion`, `coordinates`, `api`, `io-services`, etc. This makes it easier for a new developer to find relevant code and understand its scope.
*   **Comprehensive Documentation:** The code is accompanied by this Developer's Manual, which explains the "why" behind the architecture, not just the "how." Inline code comments explain specific implementation details.
*   **Headless Operation:** The firmware can be compiled and run without the display (`display` feature flag). This allows for development and testing on a wider range of hardware and simplifies debugging of the core logic without a UI.
*   **Leverage the ESP-IDF Ecosystem:** The project is built on the `esp-idf-hal` and `esp-idf-svc` crates, demonstrating how to productively build Rust applications on top of the mature Espressif IoT Development Framework. This includes using the ESP-IDF build system and FreeRTOS task management from within a safe Rust environment.
