# System Architecture Overview

The RustyScope firmware is designed with a modern, multi-threaded architecture that leverages the dual-core capabilities of the ESP32 to achieve high performance and reliability. The system is divided into several key components, each with a specific responsibility.

## Core Principles

The architecture is guided by the following principles:

*   **Real-time Performance:** The motion control system runs on a dedicated CPU core, completely isolated from other tasks like the web server and command processing. This ensures that the high-frequency stepper motor control is never interrupted.
*   **Modularity and Abstraction:** Hardware-specific code is isolated into modules with clean, high-level APIs. This makes the system easier to maintain, test, and adapt to different hardware configurations.
*   **Asynchronous and Actor-Based:** The system uses an actor-based model with message passing for communication between tasks. This avoids the need for complex locking and synchronization mechanisms, making the code safer and easier to reason about.
*   **State Management:** A centralized, thread-safe `RustyScopeState` struct holds the current state of the mount. This provides a single source of truth for all components of the system.

## Key Components

Here's a high-level overview of the main components of the system:

*   **`SystemServices`:** This struct is responsible for initializing and managing all the core hardware and system services, such as the GPS, stepper motor drivers, storage, and Wi-Fi.
*   **`RustyScope`:** This is the main controller for the telescope. It contains the high-level logic for GoTo, Sync, and Tracking, and it manages the state of the mount.
*   **The Commander Task:** This is a dedicated FreeRTOS task that acts as the central command processor for the system. It receives commands from the web server and other sources, and it orchestrates the execution of those commands.
*   **The Alpaca Server Task:** This task runs the web server that implements the ASCOM Alpaca API. It handles incoming requests from clients and translates them into commands for the Commander task.
*   **The Motion Planner and ISR:** The `MotionPlanner` is responsible for generating the step and direction signals for the stepper motors. It runs in a high-priority interrupt service routine (ISR) that is triggered by a hardware timer. This ensures that the stepper motors are driven with precise timing, resulting in smooth and accurate motion.

## Task and Core Affinity

The system is designed to take full advantage of the ESP32's dual-core architecture:

*   **Core 0 (The "Real-time Core"):** This core is dedicated to the high-priority motion control ISR. This ensures that the stepper motor control is never preempted by other tasks.
*   **Core 1 (The "Application Core"):** This core runs the main application logic, including the Commander task, the Alpaca server task, and other system services.

This separation of concerns is critical for achieving the high level of performance and reliability required for a high-precision telescope controller.