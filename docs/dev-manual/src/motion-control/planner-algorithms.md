# Deep Dive: The Motion Planner's Algorithms

The `MotionPlanner` is the core of the motion control system. It is responsible for taking high-level commands and translating them into a precise sequence of step and direction signals for the `StepperDriver`. The planner's logic is based on two key algorithms: a Digital Differential Analyzer (DDA) for multi-axis coordination and a trapezoidal motion profile for smooth acceleration and deceleration.

## The Digital Differential Analyzer (DDA)

The DDA is a classic algorithm for drawing lines on a grid, but it is also perfectly suited for controlling the motion of stepper motors. The goal is to move multiple motors (axes) simultaneously at different rates so that they all start and stop at the same time, resulting in a straight line of motion in multi-dimensional space.

The planner uses a 128-bit fixed-point implementation of the DDA. Here's how it works:

1.  **Step Counters:** The planner maintains a `step_counter` for each axis. This is a large integer that accumulates value over time.

2.  **Step Increments:** For each axis, a `step_increment` is calculated. This value represents the desired speed of the motor. A larger increment means a faster speed.

3.  **The ISR Loop:** The `update()` method of the planner is called from a high-frequency ISR (e.g., at 5 kHz). In each call, the `step_increment` for each axis is added to its `step_counter`.

    ```rust
    // Simplified DDA logic in the ISR
    for i in 0..NUM_AXES {
        self.step_counter[i] += self.step_increment[i];
    }
    ```

4.  **Checking for Overflow:** After updating the counters, the planner checks if any of them have "overflowed" a certain threshold. In this implementation, the threshold is half the maximum value of the 128-bit counter.

5.  **Generating a Step:** If a counter has overflowed, it means that the motor for that axis is due to take a step. The planner signals the driver to step that motor, and then it subtracts the threshold value from the counter to reset it for the next cycle.

    ```rust
    // Simplified overflow and step generation
    let mut steps_to_take: u8 = 0;
    for i in 0..NUM_AXES {
        if self.step_counter[i] > DDA_THRESHOLD {
            steps_to_take |= 1 << i; // Set the bit for this axis
            self.step_counter[i] -= DDA_THRESHOLD;
        }
    }
    self.driver.write(directions, steps_to_take);
    ```

By using very large 128-bit integers for the counters and increments, this algorithm can represent a vast range of speeds with extremely high precision, which is essential for smooth tracking and slewing.

## Trapezoidal Motion Profiling

If the planner were to instantly switch from zero speed to full speed, the abrupt change in velocity would cause a significant jerk in the telescope mount, which is mechanically undesirable and could cause the motors to lose steps. To solve this, the planner implements trapezoidal motion profiling.

When a new slew command is received, the planner doesn't immediately set the DDA's `step_increment` to the final target value. Instead, it ramps it up over time.

1.  **Acceleration Phase:** In each ISR tick, the planner slightly increases the `step_increment` for each axis. This causes the motors to smoothly accelerate.

2.  **Constant Speed Phase:** Once the motors reach their maximum target speed, the `step_increment` is held constant for the majority of the slew.

3.  **Deceleration Phase:** The planner continuously calculates the distance remaining to the target position. When the remaining distance is equal to the distance it took to accelerate, it begins the deceleration phase. In each ISR tick, it now slightly *decreases* the `step_increment`, bringing the motors to a smooth stop precisely at the target position.

This acceleration and deceleration creates a velocity profile that looks like a trapezoid, hence the name.

![Trapezoidal Motion Profile](https://upload.wikimedia.org/wikipedia/commons/thumb/4/44/Trapezoidal_velocity_profile.svg/400px-Trapezoidal_velocity_profile.svg.png)

By combining the high-precision DDA algorithm for multi-axis coordination with trapezoidal motion profiling for smooth acceleration, the `MotionPlanner` can execute fast, precise, and mechanically smooth movements, which is fundamental to the performance of the RustyScope controller.
