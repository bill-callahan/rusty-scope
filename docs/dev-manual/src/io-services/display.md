# The Display Controller and UI

RustyScope is designed primarily for "headless" operation, but it includes an optional display feature for providing a local status dashboard. This is managed by the `DisplayController` and the main application loop, which work together to create an efficient and responsive user interface.

This feature is controlled at compile-time by the `display` feature flag.

## The `DisplayController`

The `DisplayController` struct, defined in `src/ts35/mod.rs`, is a high-level wrapper that encapsulates all the logic for interacting with the 3.5" TFT display. It manages the low-level details of the SPI communication, GPIO control (for chip select, reset, and data/command signals), and the drawing primitives.

Its key responsibilities include:

*   **Initialization:** It configures the SPI peripheral and the necessary GPIO pins, and it sends the required command sequence to initialize the display hardware.
*   **Drawing Primitives:** It provides high-level methods for drawing on the screen, such as `clear_screen()`, `draw_text_line()`, and `display_splash_16bit()`.
*   **State Management:** It is wrapped in an `Arc<Mutex<>>` within the `SystemServices` struct, allowing it to be safely accessed from the main task.

## The Dashboard UI Loop

When the `display` feature is enabled, the main application loop in `src/main.rs` transforms into a UI rendering loop. This loop is responsible for creating the dashboard and keeping it updated with the latest system telemetry.

To ensure the display is updated efficiently and without flickering, the loop implements a **manual double-buffering and diffing** technique.

Here is the process, which repeats continuously:

1.  **Fetch Data:** The loop begins by gathering the latest data from all relevant system components (the motion planner, the GPS controller, the time manager, etc.).

2.  **Generate Current State:** This raw data is then used to populate a `DisplayState` struct. This struct holds the formatted strings for every piece of information that *should* be on the screen for the current frame (e.g., `axis_x_string`, `utc_string`, `status_string`).

3.  **Compare and Draw (Diffing):** The `current_state` is then compared, field by field, against the `previous_state` from the last iteration of the loop. A line of text is only redrawn on the display if its content has actually changed.

    ```rust
    // Simplified diffing logic from main.rs

    // Compare the UTC string from the current frame to the previous one
    if current_state.utc_string != previous_state.utc_string {
        // If they are different, redraw that specific line on the screen
        display_controller.draw_text_line(
            &current_state.utc_string, 
            Point::new(20, 170), 
            // ... style info ...
        )?;
    }
    ```

4.  **Update State:** Finally, the `current_state` is saved as the `previous_state` for the next iteration of the loop.

This diffing mechanism is highly effective. In any given moment, most of the information on the dashboard is static. By only redrawing the elements that have changed, the loop minimizes the amount of data sent over the SPI bus and avoids any perceptible flicker, resulting in a clean and professional-looking interface.
