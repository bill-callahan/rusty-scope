# RustyScope: A High-Precision Telescope Controller

Well... 

Imagine combining the thrill of stargazing with the satisfaction of building something from scratch. That's exactly what I've set out to do.

**RustyScope is a high-performance, open-source telescope and focuser controller built entirely in Rust, running on the powerful and affordable ESP32 microcontroller.** It is fully compatible with the modern **ASCOM Alpaca** standard, allowing it to be controlled by popular astrophotography software like N.I.N.A., PHD2, and Stellarium right out of the box.

This project was born from a passion for astronomy and a desire to create a truly open, high-precision, and community-driven alternative to expensive, closed-source commercial controllers.
## Journal

Checkout my journal here - [Journal](https://bill-callahan.github.io/rusty-scope/journal/) 

--

## ✨ Key Features

RustyScope is packed with features typically found only in high-end commercial mounts:

*   **High-Precision Tracking:** The motion planner uses a high-frequency (5 kHz) interrupt service routine and a 128-bit fixed-point Digital Differential Analyzer (DDA) to achieve incredibly smooth and accurate tracking rates.
*   **GPS-Disciplined Timing:** An integrated GPS module with **Pulse-Per-Second (PPS)** support is used to continuously discipline the ESP32's internal clock against an atomic time standard. This eliminates tracking errors caused by crystal drift and temperature changes.
*   **Intelligent GoTo Slewing:** A **4-quadrant, shortest-path** slew calculator understands the geometry of a German Equatorial Mount, ensuring fast, efficient, and physically correct slews that avoid pier crashes and unnecessary rotations.
*   **Full Alpaca Compatibility:** Provides a built-in web server that implements the ASCOM Alpaca standards for `Telescope` and `Focuser` devices, including UDP discovery. It just works.
*   **Robust Multi-Tasking Architecture:** Built on FreeRTOS, the system dedicates a full CPU core to real-time motion control, completely isolated from networking and other application tasks for maximum stability.
*   **"Headless" Operation:** The controller is designed to run in a lean, headless mode for maximum reliability. An optional display feature can be enabled for local status monitoring.
*   **Advanced Pointing Model:** The software correctly handles hemisphere-agnostic pole crossing ("pole flips"), ensuring full sky coverage.
*   **Persistent Configuration:** All mount settings are saved to the internal flash filesystem, preserving your calibration across reboots.
*   **Truly Open Source:** Licensed under the **GPLv3**, ensuring that the project and any derivatives will always remain free and open for the community to use, modify, and improve.

---

## 🛠️ Hardware

This project is designed around the affordable and powerful **Makerbase MKS DLC32** controller board, which provides an excellent all-in-one platform with integrated stepper drivers, power management, and I/O.

A complete bill of materials and wiring guide can be found in the [**Hardware Guide**] section of the documentation.

*   **Microcontroller:** ESP32 (Dual Core)
*   **Motion Control:** 74HC595 Shift Register with standard Step/Dir motor drivers
*   **Timing & Location:** u-blox NEO-7M GPS Module with PPS
*   **Storage:** Onboard SPIFFS and external SD Card support

---

## 🤝 Contributing

This is a passion project, and contributions are welcome! Whether it's reporting a bug, suggesting a feature, improving the documentation, or submitting a pull request, your help is appreciated.