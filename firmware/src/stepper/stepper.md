
# Stepper Motion Control (`stepper/`)

The `stepper` module is the real-time motion control engine of the RustyScope project. It is responsible for the precise, coordinated, and smooth movement of the telescope's motors. The architecture is a classic three-layer stack, separating hardware control, motion planning, and high-level commands.

## 1. Core Responsibilities

*   **Pulse Generation:** Generate the precise, high-frequency electrical pulse trains required to drive the stepper motors.
*   **Motion Planning:** Implement acceleration and deceleration (trapezoidal profiles) to ensure smooth starts and stops, preventing mechanical shock and lost steps.
*   **Multi-Axis Coordination:** Interpolate movement across multiple axes simultaneously (e.g., RA and Dec) to produce a straight-line slew from a start point to a target.
*   **Continuous Rate Control:** Drive motors at a constant, precise rate for sidereal tracking and guiding.

## 2. Architecture: A Three-Layer Real-Time Stack

The motion control system is divided into three distinct components, each with a clear responsibility.

```mermaid
flowchart TD
    subgraph "High-Level Controller (Commander Task)"
        A[<b>RustyScope Controller</b><br>`execute_slew_to_target()`<br>`execute_pulse_guide()`]
    end

    subgraph "Stepper Module"
        subgraph "Layer 2: The Planner"
            B[<b>MotionPlanner (`planner.rs`)</b><br>State Machine (Idle, Moving, Tracking)<br>Bresenham/DDA Algorithms<br>Acceleration Profiles]
        end

        subgraph "Layer 1: The Driver"
            C[<b>I2sStepperDriver (`i2s0.rs`)</b><br>Hardware Abstraction<br>Manages I2S Peripheral & DMA]
        end

        subgraph "Layer 0: The Hardware Interface"
            D[<b>MotorState (`sr74hc595.rs`)</b><br>Bit-level control<br>Defines pulse/direction bitmasks]
        end
    end

    subgraph "Physical Hardware"
        E[ESP32 I2S Peripheral]
        F[74HC595 Shift Register]
        G[Stepper Motor Drivers]
    end

    A -- "move_to_steps([x,y,z])" --> B
    A -- "track_rate_sps([x,y,z])" --> B

    B -- "send_steps(&[u8])" --> C
    C -- Writes to DMA --> E
    E -- Bit-bangs serial data --> F
    F -- Controls --> G

    B -- uses --> D
```

### 2.1. Layer 0: Hardware Interface (`sr74hc595.rs`)

This is the lowest level of the software stack, defining the bit-level interface to the 74HC595 shift register that controls the motor driver hardware.

*   **`MotorState` Struct:** This struct holds a single `u8` value. Each bit in this byte corresponds to a specific output pin on the shift register (e.g., X_STEP, X_DIR, Y_STEP, Y_DIR, ENABLE).
*   **Bit-Masking API:** It provides a "fluent" API for manipulating the bits in a safe and readable way (e.g., `base_state.step_x().set_y_dir(true)`). This abstracts away raw bitwise operations (`|`, `&`, `^`) from the rest of the code.
*   **Purpose:** This layer translates a logical intent like "pulse the X-axis motor" into the exact byte (`0b00000001`) that needs to be sent to the hardware.

### 2.2. Layer 1: The Driver (`i2s0.rs`)

The `I2sStepperDriver` is a lean hardware abstraction layer (HAL) for the ESP32's I2S peripheral.

*   **I2S for Stepper Control:** This is an advanced technique. Instead of using the CPU to manually "bit-bang" pulses in a tight loop (which is inefficient and prone to jitter), RustyScope offloads this task to the I2S hardware peripheral.
*   **DMA (Direct Memory Access):** The driver's primary method, `send_steps(&[u8])`, writes a buffer of bytes directly to the I2S peripheral's DMA engine. The DMA then automatically clocks these bytes out to the shift register at a precise, constant sample rate, freeing the CPU entirely.
*   **Purpose:** To provide a simple, high-throughput channel for sending a stream of `MotorState` bytes to the hardware without CPU intervention.

### 2.3. Layer 2: The Planner (`planner.rs`)

The `MotionPlanner` is the intelligent heart of the motion system. It runs in the context of a high-frequency `esp_timer` interrupt service routine (ISR).

*   **ISR-Driven:** The `planner.update()` method is called thousands of times per second by a hardware timer. This ensures precise, deterministic timing for motor control.
*   **State Machine (`PlannerState`):** The planner operates as a state machine with key states:
    *   `Idle`: Doing nothing.
    *   `Moving(MoveState)`: Executing a finite, point-to-point slew with acceleration.
    *   `Tracking { rates }`: Executing a continuous, constant-rate move for sidereal tracking.
    *   `Guiding { rates }`: A specialized continuous move state for guiding.
*   **Dual-Algorithm Approach:** The planner intelligently uses two different algorithms depending on the state:
    1.  **Bresenham's Line Algorithm (for `Moving` state):** This classic algorithm is used for multi-axis slews. It perfectly interpolates steps across the RA and Dec axes to ensure the telescope moves in a straight line. It is combined with trapezoidal acceleration logic to calculate the speed at each step of the move.
    2.  **Digital Differential Analyzer (DDA) (for `Tracking` state):** This algorithm uses fixed-point arithmetic (`rate_accumulators`) to generate pulses at a precise, and often fractional, average rate. This is ideal for the slow, constant-speed movement required for sidereal tracking.

## 3. Data Flow Example: A Single Step in a Slew

1.  **Trigger:** The `esp_timer` ISR fires.
2.  **`planner.update()`:** The callback function is invoked.
3.  **State Check:** The planner sees it is in the `Moving(move_state)` state.
4.  **Acceleration Calculation:** It calculates the current target speed based on whether it's in the acceleration, cruise, or deceleration phase of the move.
5.  **Bresenham's Logic:** It uses the error terms for each axis to decide which motor(s) need to be pulsed in this specific ISR tick to maintain a straight path.
6.  **Bitmask Generation:** It uses the `MotorState` API to construct the correct byte for the step pulse (e.g., `base_state.step_x().step_y()`).
7.  **DMA Write:** It calls `driver.send_steps()` with a two-byte buffer: `[step_high_state, step_low_state]`.
8.  **Hardware Execution:** The I2S DMA engine receives these two bytes and automatically clocks them out to the shift register, generating the physical step pulse on the motor driver pins.
9.  **State Update:** The planner updates its internal `current_pos` and error term variables and waits for the next ISR tick.

## 4. Key Design Decisions

*   **Offloading to Hardware (I2S/DMA):** This is the single most important architectural decision. It allows for extremely high step rates and jitter-free motion, even while the CPU is busy serving web requests or updating the display.
*   **ISR Context:** Running the core planning logic in a high-priority timer ISR ensures timing is predictable and independent of other application tasks.
*   **`try_lock()` Safety:** The ISR callback uses `try_lock()` when accessing the Planner's mutex. This is a critical safety feature that prevents deadlocks if another task (like the commander) happens to be holding the lock when the interrupt fires.
*   **Stateful Planner:** The planner is not just a simple pulse generator; it's a stateful object that knows the full profile of a move from start to finish. This enables complex features like acceleration and multi-axis interpolation.
*   **Flexible State Machine:** The planner's state machine is not just a simple on/off switch. It can be in multiple states simultaneously (e.g., moving and guiding). This allows for complex, multi-axis moves like slews that involve both acceleration and constant-speed tracking.