# Stepper Motion Control (`stepper/`)

The `stepper` module is the real-time motion control engine of the RustyScope project. It is responsible for the precise, coordinated, and smooth movement of the telescope's motors. The architecture is a three-layer stack, separating hardware control, motion planning, and high-level commands.

## 1. Core Responsibilities

*   **Pulse Generation:** Generate the precise, high-frequency electrical pulse trains required to drive the stepper motors via LV8729 drivers.
*   **Multi-Axis Coordination:** Interpolate movement across multiple axes simultaneously (e.g., RA and Dec) to produce a straight-line slew from a start point to a target using Bresenham's line algorithm.
*   **Continuous Rate Control:** Drive motors at a constant, precise rate for sidereal tracking and guiding using a Digital Differential Analyzer (DDA).
*   **Hardware Abstraction:** Provide a clean interface between the motion planning logic and the 74HC595 shift register hardware.

## 2. Architecture: A Three-Layer Real-Time Stack

The motion control system is divided into three distinct components, each with a clear responsibility.

### 2.1. Layer 0: Hardware Interface (`sr74hc595.rs`)

This is the lowest level of the software stack, defining the bit-level interface to the 74HC595 shift register that controls the motor driver hardware.

*   **`MotorState` Struct:** This struct holds a single `u8` value. Each bit in this byte corresponds to a specific output pin on the shift register (e.g., X_STEP, X_DIR, Y_STEP, Y_DIR, ENABLE).
*   **Bit-Masking API:** It provides a "fluent" API for manipulating the bits in a safe and readable way (e.g., `base_state.step_x().set_y_dir(true)`). This abstracts away raw bitwise operations (`|`, `&`, `^`) from the rest of the code.
*   **Simultaneous Multi-Axis Stepping:** The `step_x()`, `step_y()`, and `step_z()` methods only OR their respective step bit into the state. They do not clear other step bits, allowing multiple axes to be pulsed simultaneously on the same ISR tick. This is critical for smooth diagonal movement via Bresenham interpolation.
*   **Purpose:** This layer translates a logical intent like "pulse the X-axis motor" into the exact byte (`0b00000010`) that needs to be sent to the hardware.

**Shift Register Pin Mapping:**

| Bit | Pin Name   | Function              |
|-----|------------|-----------------------|
| Q0  | ENABLE_PIN | Motor Enable (Active Low) |
| Q1  | X_STEP_PIN | X-Axis Step Pulse     |
| Q2  | X_DIR_PIN  | X-Axis Direction      |
| Q3  | Z_STEP_PIN | Z-Axis Step Pulse     |
| Q4  | Z_DIR_PIN  | Z-Axis Direction      |
| Q5  | Y_STEP_PIN | Y-Axis Step Pulse     |
| Q6  | Y_DIR_PIN  | Y-Axis Direction      |
| Q7  | BEEPER_PIN | Beeper Control        |

### 2.2. Layer 1: The Driver (`bitbang_driver.rs`)

The `BitbangStepperDriver` is a software-driven ("bit-banged") hardware abstraction layer for the 74HC595 shift register.

*   **Bit-Banged SPI-like Protocol:** The driver manually toggles three GPIO pins (SCLK, MOSI, LATCH) to shift data into the 74HC595. This approach provides cycle-accurate control over timing, which is essential for generating reliable step pulses.
*   **IRAM Placement:** All critical functions (`write_byte`, `send_steps`) are placed in `.iram1.text` to ensure they execute from instruction RAM. This eliminates cache-miss jitter and ensures deterministic timing even under heavy CPU load.
*   **Pulse Generation:** The `send_steps(&[u8])` method accepts a two-byte buffer `[step_high_state, step_low_state]`. It shifts and latches the high byte (causing the step pins to go HIGH), then shifts and latches the low byte (causing the step pins to go LOW). This sequence creates a clean 1µs hardware pulse on the LV8729 driver inputs.
*   **Timing Margins:** Each bit shift includes a 1µs setup delay before the clock edge, comfortably exceeding the 74HC595's 500ns data setup time requirement at 3.3V. The 1µs pulse width exceeds the LV8729 driver's 500ns minimum step pulse requirement.
*   **Purpose:** To provide a simple, reliable, and precisely-timed interface for sending `MotorState` bytes to the shift register hardware.

### 2.3. Layer 2: The Planner (`planner.rs`)

The `MotionPlanner` is the intelligent heart of the motion system. It runs in the context of a high-frequency hardware timer interrupt service routine (ISR).

*   **ISR-Driven:** The `planner.update()` method is called thousands of times per second by a hardware timer. This ensures precise, deterministic timing for motor control.
*   **State Machine (`PlannerMode`):** The planner operates as a state machine with four states:
    *   `Idle`: No motion is occurring.
    *   `Moving`: Executing a finite, point-to-point slew using Bresenham's algorithm.
    *   `Tracking`: Executing a continuous, constant-rate move for sidereal tracking.
    *   `Guiding`: A specialized continuous-rate move for autoguiding corrections.
*   **Dual-Algorithm Approach:** The planner uses two different algorithms depending on the state:
    1.  **Bresenham's Line Algorithm (for `Moving` state):** This classic algorithm is used for multi-axis slews. It interpolates steps across all axes to ensure the telescope moves in a straight line from the current position to the target. The dominant axis steps on every tick, while non-dominant axes step conditionally based on accumulated error terms.
    2.  **Digital Differential Analyzer (DDA) (for `Tracking`/`Guiding` states):** This algorithm uses fixed-point arithmetic with 128-bit accumulators to generate pulses at a precise, fractional average rate. This is ideal for the slow, constant-speed movement required for sidereal tracking and guiding corrections.

## 3. Data Flow Example: A Single Step in a Slew

1.  **Trigger:** The hardware timer ISR fires at the configured update frequency.
2.  **`planner.update()`:** The ISR callback invokes the planner's update method.
3.  **State Check:** The planner sees it is in the `Moving` state.
4.  **Bresenham's Logic:** It uses the error terms for each axis to decide which motor(s) need to be pulsed in this specific ISR tick to maintain a straight path. Multiple axes may step simultaneously.
5.  **Bitmask Generation:** It uses the `MotorState` API to construct the correct byte for the step pulse (e.g., `base_state.step_x().step_y()` sets both X and Y step bits high).
6.  **Driver Call:** It calls `driver.send_steps()` with a two-byte buffer: `[step_high_state, step_low_state]`.
7.  **Hardware Execution:** The bitbang driver shifts the high byte into the 74HC595 and latches it (step pins go HIGH). It then shifts the low byte and latches it (step pins go LOW). This creates a clean 1µs pulse on the LV8729 driver inputs.
8.  **State Update:** The planner updates its internal `current_pos`, slew counters, and Bresenham error terms, then returns control to the ISR.

## 4. Key Design Decisions

*   **Bit-Banged Driver with IRAM Placement:** Rather than offloading to a hardware peripheral (like I2S or SPI), the system uses a software bit-bang approach with all critical code placed in IRAM. This provides cycle-accurate timing control, eliminates DMA complexity, and ensures deterministic behavior. The ESP32's 240MHz clock speed makes this approach more than sufficient for the required step rates.
*   **ISR Context:** Running the core planning logic in a high-priority timer ISR ensures timing is predictable and independent of other application tasks (WiFi, web server, display updates).
*   **Single Shift Register Design:** All motor control signals (step, direction, enable) are multiplexed through a single 74HC595 shift register. This minimizes GPIO usage on the ESP32 while providing sufficient pins for three axes plus auxiliary functions.
*   **Stateful Planner:** The planner is not just a simple pulse generator; it is a stateful object that tracks the full context of a move (target position, accumulated error, current position). This enables complex features like multi-axis interpolation and seamless transitions between tracking and guiding.
*   **Fixed-Point DDA for Tracking:** The continuous tracking modes use 128-bit integer accumulators instead of floating-point math in the ISR. This avoids expensive FPU operations in interrupt context while maintaining sub-microstep precision over long tracking durations.
*   **Seamless Guiding Transitions:** When transitioning from `Tracking` to `Guiding`, the DDA accumulators are preserved (not reset). This maintains fractional step error continuity, ensuring no positional discontinuity occurs when a guide correction is applied.
