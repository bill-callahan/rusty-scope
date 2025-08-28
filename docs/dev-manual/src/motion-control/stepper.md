# Motion Control Stack Overview

The motion control system is the real-time heart of the RustyScope controller. It is responsible for generating the precise, high-frequency electrical pulses that drive the stepper motors, which in turn move the telescope mount. The entire system is designed to be as accurate and jitter-free as possible, which is critical for long-exposure astrophotography.

The stack is composed of three main layers:

1.  **The Motion Planner (`MotionPlanner`)**: The high-level brain of the operation.
2.  **The Stepper Driver Trait (`StepperDriver`)**: An abstraction layer that defines a common interface for different hardware driving techniques.
3.  **The Concrete Driver Implementations**: The low-level code that directly manipulates the hardware to generate step pulses (e.g., `I2sStepperDriver`, `BitbangStepperDriver`).

## 1. The Motion Planner

The `MotionPlanner` is the central component that orchestrates all movement. It does not know or care *how* the step pulses are physically generated; it only deals in the logic of motion.

Its key responsibilities are:

*   **Position Tracking:** It maintains the current absolute position of each motor axis in steps.
*   **Targeting:** It accepts high-level commands like "move to position X" or "track at rate Y".
*   **Path Planning:** It calculates the most efficient path to get from the current position to the target position.
*   **Acceleration and Deceleration:** It implements trapezoidal motion profiling, meaning it smoothly accelerates the motors up to a maximum speed and then smoothly decelerates them as they approach the target. This prevents jerky movements and reduces mechanical stress.
*   **Digital Differential Analyzer (DDA):** It uses a 128-bit fixed-point DDA algorithm to calculate the precise timing for each step pulse for each motor, ensuring that all axes move in a perfectly coordinated way.

The planner is executed within a high-priority, high-frequency timer Interrupt Service Routine (ISR). In each ISR tick (e.g., every 200 microseconds), the planner's `update()` method is called, which performs the DDA calculation and tells the driver layer which motors need to take a step.

## 2. The `StepperDriver` Trait

To decouple the planner from the hardware, we use a `StepperDriver` trait. This trait defines a simple, abstract interface that any hardware driver must implement.

```rust
// A simplified representation
pub trait StepperDriver {
    fn begin(&mut self);
    fn end(&mut self);
    fn write(&mut self, directions: u8, steps: u8);
}
```

The `write` method is the most important. The planner calls this method from the ISR, passing it a bitmask that indicates which motors should step (`steps`) and in which direction (`directions`). It is the driver's job to take this information and translate it into the appropriate electrical signals.

## 3. Concrete Driver Implementations

This is where the hardware-specific logic lives. RustyScope can be compiled with different drivers, but the most important one is the `I2sStepperDriver`.

*   **`I2sStepperDriver`**: This is a highly novel approach that uses the ESP32's I2S (Inter-IC Sound) peripheral, a hardware block normally used for audio, to generate extremely stable, jitter-free step pulses. This is the key to the system's high precision.
*   **`BitbangStepperDriver`**: A simpler, software-only implementation that directly toggles the GPIO pins. This is less precise than the I2S driver but is useful for testing and debugging on hardware that may not have the I2S peripheral available or correctly wired.
*   **`SpiStepperDriver`**: An implementation that uses the SPI peripheral to send step data to external shift registers. This is another common technique for driving multiple stepper motors.

By using this layered architecture, the high-level motion planning logic is completely independent of the low-level hardware control. This makes it easy to add support for new types of stepper driver hardware in the future without having to modify the core planning and DDA algorithms.
