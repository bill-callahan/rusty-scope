# ESP32 Firmware

This is a firmware for the ESP32 microcontroller written in Rust.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [ESP-IDF](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/index.html)
- [cargo-espflash](https://github.com/esp-rs/espflash)

## Getting Started

### Clone the Repository

```bash
git clone https://github.com/bill-callahan/rusty-scope.git
cd rusty-scope/firmware
```

### Configuration

1. Copy the example configuration file:

```bash
cp .env_example .env
```

2. Edit the configuration file with your settings:

```bash
vi .env
```
..or just use your favorite editor.

Adjust WiFi credentials, MQTT settings, and other parameters as needed.

## Building the Firmware

1. Set up the Rust toolchain for ESP32:

```bash
espup install --toolchain=1.93.0.0
```
and make sure you source your export-esh.sh script before building.  This is Muey Importante!!
```bash
. ~/export-esp.sh
```

2. Build the firmware:

```bash
cargo build --release
```

## Flashing to ESP32

Connect your ESP32 device to your computer via USB, then:

```bash
cargo run --release
```



## Troubleshooting

If you encounter issues with the build or flash process:

- Make sure your ESP-IDF is properly installed and configured (currently, 1.93.0.0 works)
- Check if your ESP32 is properly connected
- Verify that you have the correct permissions to access the serial port

## License

This project is licensed under the MIT License - see the LICENSE file for details.
