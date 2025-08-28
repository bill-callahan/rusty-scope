# Persistence: The Storage Layer

A telescope mount has many configuration parameters that need to be preserved even when the device is powered off. This includes everything from the mount's physical characteristics (gear ratios, steps per revolution) to user settings (site latitude/longitude, guide rates). The storage layer in RustyScope is responsible for managing this persistent data.

## The `StorageManager`

The `StorageManager`, defined in `src/storage/mod.rs`, provides a clean, high-level API for all configuration management. It abstracts away the underlying details of the filesystem, providing a simple interface for the rest of the application.

Its primary responsibilities are:

*   **Loading Configuration:** On startup, it loads the configuration file from the filesystem. If the file doesn't exist or is corrupted, it creates a new one with default values.
*   **Saving Configuration:** It provides a method to serialize the current configuration and write it back to the filesystem.
*   **Deleting Configuration:** It provides a method to delete the configuration file, which is used for performing a factory reset.

## The Filesystem: SPIFFS

The `StorageManager` uses the **SPIFFS (SPI Flash File System)** as its backend. SPIFFS is a lightweight filesystem designed specifically for embedded systems that use NOR flash memory, like the ESP32. It is wear-leveling, which means it distributes writes across the flash memory to prolong its life.

The low-level interaction with SPIFFS is handled by the `SpiffsManager` struct (`src/storage/spiffs.rs`), which the `StorageManager` uses internally. This separation of concerns means that the main application logic doesn't need to know anything about SPIFFS; it only interacts with the `StorageManager`.

## The `DeviceConfig` Struct

The data itself is stored in a single `DeviceConfig` struct. This struct holds all the persistent parameters for the device.

```rust
// A simplified DeviceConfig struct
#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct DeviceConfig {
    pub configured: bool,
    pub motorx_spr: u32,
    pub motorx_microsteps: u32,
    pub motorx_gratio: f64,
    pub site_longitude: f64,
    pub site_latitude: f64,
    pub guide_ra_rate_dps: f32,
    // ... and many other fields
}
```

This struct uses the `serde` library to automatically handle serialization and deserialization to and from the JSON format. When the `StorageManager` saves the configuration, it serializes the `DeviceConfig` struct into a JSON string and writes it to a file named `config.json` in the SPIFFS filesystem.

When it loads the configuration, it reads the JSON file from SPIFFS and deserializes it back into a `DeviceConfig` struct.

This approach provides a robust and flexible way to manage the device's configuration. Adding a new persistent parameter is as simple as adding a new field to the `DeviceConfig` struct; `serde` and the `StorageManager` handle the rest.
