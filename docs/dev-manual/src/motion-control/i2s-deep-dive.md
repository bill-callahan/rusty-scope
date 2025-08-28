# Deep Dive: I2S for Jitter-Free Stepping

One of the most innovative aspects of the RustyScope controller is its use of the ESP32's I2S (Inter-IC Sound) peripheral to generate the stepper motor pulses. This is a non-obvious use of the hardware, but it provides a significant advantage: **extremely precise, jitter-free timing** that is nearly impossible to achieve with software-based methods.

## The Problem with Software-Based Stepping

A naive approach to generating step pulses is to simply use a software loop that toggles a GPIO pin high and then low.

```rust
// Simplified bit-banging
loop {
    pin.set_high();
    delay_microseconds(100);
    pin.set_low();
    delay_microseconds(100);
}
```

The problem with this method is **jitter**. The `delay_microseconds` function is not perfect. The actual delay will be affected by other tasks running on the system, interrupts firing, and other system-level activities. This means the time between step pulses will not be perfectly consistent. For a telescope mount, this jitter translates directly into tracking errors, resulting in trailed stars in long-exposure photographs.

## The I2S Solution

The I2S peripheral is designed to send digital audio data to a DAC (Digital-to-Analog Converter). It consists of a serializer that reads data from a DMA (Direct Memory Access) buffer and shifts it out on a data pin at a precise, configurable clock rate. The key insight is that we can hijack this functionality to create a stream of perfectly timed digital pulses.

Here's how it works:

1.  **DMA Buffer as a Pulse Pattern:** We create a large buffer in memory that is accessible by the DMA controller. Instead of filling this buffer with audio samples, we fill it with a repeating pattern of bytes that represents our step pulses. For example, a `0b10101010` byte would represent four high/low transitions (four steps).

2.  **I2S as a High-Speed Clock:** We configure the I2S peripheral to read from this DMA buffer and shift the data out at a very high frequency (e.g., 100 kHz). The I2S hardware clock is extremely stable and is not affected by software jitter.

3.  **Connecting the Dots:** The `I2sStepperDriver` takes the step commands from the `MotionPlanner` (e.g., "step motor X and motor Z"). It translates this into a single byte where each bit corresponds to a motor's step pin. For example, if motor X is on bit 0 and motor Z is on bit 2, the byte would be `0b00000101`. This byte is then written into the DMA buffer.

4.  **Continuous, Jitter-Free Output:** The I2S peripheral continuously reads from this DMA buffer and streams the bits out on its data pin. Because the I2S clock is hardware-driven, the time between each bit (and therefore each step pulse) is perfectly consistent, eliminating software-induced jitter.

## The `I2sStepperDriver`

The `I2sStepperDriver` in `src/stepper/i2s0.rs` encapsulates this logic. Its `write` method is called from the high-frequency motion ISR.

```rust
// In I2sStepperDriver
fn write(&mut self, directions: u8, steps: u8) {
    // 1. Combine direction and step data with the latch signal
    let output_byte = (self.latch_pin_mask)
        | (directions & self.direction_pins_mask)
        | (steps & self.step_pins_mask);

    // 2. Write the byte to the I2S DMA buffer
    self.dma_buffer[self.buffer_position] = output_byte;
    
    // 3. Write a zero byte for the "low" part of the pulse
    self.dma_buffer[self.buffer_position + 1] = 0;

    // 4. Advance the buffer position
    self.buffer_position = (self.buffer_position + 2) % self.dma_buffer.len();
}
```

This implementation is highly efficient. The ISR simply has to calculate a single byte and write it into a buffer. The I2S hardware and DMA controller handle the rest, ensuring that the mount's motors receive a clean, stable, and precise stream of step pulses. This is a cornerstone of RustyScope's high-precision tracking capability.
