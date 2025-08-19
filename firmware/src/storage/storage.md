
# Storage Manager (`storage/`)

The Storage Manager is the persistence layer of the RustyScope project. Its primary role is to save and load the telescope's configuration, ensuring that critical settings like motor calibration, gear ratios, and site information are preserved even when the device is powered off. The architecture is a two-layer system designed for robustness, safety, and ease of use.

## 1. Core Responsibilities

*   **Persistence:** Save critical device configuration to non-volatile flash memory.
*   **Configuration Loading:** Load the configuration from flash memory during system startup.
*   **Robustness:** Handle scenarios where the filesystem may not exist or be corrupted (e.g., on first boot), by automatically formatting it.
*   **Abstraction:** Provide a simple, high-level API (`save_config`, `load_config`) that hides the complexities of filesystems, file paths, and data serialization from the rest of the application.

## 2. Architecture: A Safe, Layered Approach

The storage system is composed of a high-level "Manager" that uses a low-level, hardware-specific "Driver" or "Guard".

```mermaid
flowchart TD
    subgraph "Application Layer"
        A[<b>RustyScope Controller</b>]
    end

    subgraph "Storage Module"
        subgraph "Layer 1: The Manager (Logical)"
            B[<b>StorageManager (`storage/mod.rs`)</b><br>Public API (`save_config`, `load_config`)<br>Knows about `DeviceConfig` struct<br>Handles JSON serialization/deserialization]
        end

        subgraph "Layer 0: The Driver (Physical)"
            C[<b>SpiffsManager (`storage/spiffs.rs`)</b><br>RAII Guard for the filesystem<br>Handles mounting/unmounting<br>Provides basic file I/O (`write_file`, `read_file`)]
        end
    end

    subgraph "Physical Hardware & OS"
        D[ESP32 Flash Memory]
        E[ESP-IDF SPIFFS Driver (VFS)]
    end

    A -- "load_config()" --> B
    B -- "read_file(...)" --> C
    C -- `std::fs::File::open` --> E
    E -- reads from --> D

    A -- "save_config(&config)" --> B
    B -- `serde_json::to_string` --> F[JSON String]
    B -- "write_file(...)" --> C
    C -- `std::fs::File::create` --> E
    E -- writes to --> D
```

### 2.1. Layer 0: The Filesystem Guard (`SpiffsManager`)

The `SpiffsManager` is a low-level component that directly manages the SPIFFS (SPI Flash File System) partition. Its design is centered around the **RAII (Resource Acquisition Is Initialization)** pattern, which is crucial for safety in an environment without a full operating system.

*   **Resource Acquisition (`SpiffsManager::new`):**
    *   When a `SpiffsManager` is created, it makes an `unsafe` FFI (Foreign Function Interface) call to the underlying ESP-IDF C function `esp_vfs_spiffs_register`.
    *   A key parameter, `format_if_mount_failed: true`, is passed. This is a critical robustness feature. It tells the driver to automatically format the flash partition if it's empty or corrupted, ensuring the system can always boot to a usable state.
    *   This "registers" the SPIFFS partition with the ESP-IDF's Virtual File System (VFS), making it accessible via standard Rust file I/O operations (e.g., `std::fs::File`).
*   **Resource Release (`impl Drop for SpiffsManager`):**
    *   The `SpiffsManager` implements the `Drop` trait. This code is *guaranteed* to run when the `SpiffsManager` object goes out of scope.
    *   The `drop` function calls `esp_vfs_spiffs_unregister`, which cleanly unmounts the filesystem and frees all associated resources.
    *   This RAII pattern prevents resource leaks, which is vital for the long-term stability of an embedded system.

### 2.2. Layer 1: The Configuration Manager (`StorageManager`)

The `StorageManager` is the high-level, logical API that the rest of the application interacts with. It owns an instance of `SpiffsManager`.

*   **Abstraction:** It hides all the implementation details. The `RustyScope` controller doesn't know or care about SPIFFS, JSON, or file paths; it simply interacts with the `StorageManager`.
*   **Serialization (`save_config`):**
    *   Takes a reference to the `DeviceConfig` struct.
    *   Uses the `serde_json::to_string()` function to serialize the struct into a human-readable JSON string.
    *   Calls the underlying `spiffs.write_file()` method to save this string to a predefined path (e.g., `/spiffs/config.json`).
*   **Deserialization (`load_config`):**
    *   Calls `spiffs.read_file()` to get the JSON string from flash.
    *   Uses `serde_json::from_str()` to deserialize the JSON string back into a `DeviceConfig` struct.
    *   If the file doesn't exist, it gracefully returns a default `DeviceConfig` instance.

### 2.3. The Data Model (`DeviceConfig` struct)

This struct, defined in `storage/mod.rs`, is the schema for all persistent data.

*   **`#[derive(Serialize, Deserialize)]`:** These macros, provided by the `serde` crate, automatically generate the code required to convert the struct to and from the JSON format. This is a massive simplification compared to manual parsing.
*   **Contents:** It holds all the critical values that define the mount's physical characteristics, such as `motorx_spr` (steps per revolution), `motorx_microsteps`, and `motorx_gratio`.

## 3. Data Flow Example: Saving Configuration

1.  **Trigger:** A user saves settings via a setup webpage, or the system saves its last known position on shutdown. This results in a call to `storage_manager.save_config(&config)`.
2.  **Serialization:** The `StorageManager` calls `serde_json::to_string(&config)`, which converts the `DeviceConfig` struct into a JSON string like `{"configured":true,"motorx_spr":200,...}`.
3.  **File Write:** The `StorageManager` calls `self.spiffs.write_file("/spiffs/config.json", &json_string)`.
4.  **VFS Call:** The `SpiffsManager` uses the standard Rust function `std::fs::File::create()`. Because the SPIFFS partition was registered with the VFS, the VFS intercepts this call and redirects it to the appropriate ESP-IDF driver function.
5.  **Flash Write:** The ESP-IDF SPIFFS driver handles the low-level operations of writing the data to the correct blocks and pages on the physical flash chip.

## 4. Key Design Decisions

*   **Layered Abstraction:** The two-layer design cleanly separates logical operations (what to save) from physical operations (how to save it).
*   **RAII for Safety:** Using the `Drop` trait on `SpiffsManager` guarantees that filesystem resources are never leaked, which is essential for embedded stability.
*   **Serialization with `serde`:** Using `serde` is a cornerstone of the design. It makes the process of saving and loading complex configuration data trivial, type-safe, and robust.
*   **Format on Fail:** This is a key decision for user experience. It ensures the device is "self-healing" and can recover from a corrupted filesystem without needing to be manually reflashed.
