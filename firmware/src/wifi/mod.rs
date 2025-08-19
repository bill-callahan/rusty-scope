// src/wifi/mod.rs

// Declare the 'wifi' sub-module as private to this module.
mod wifi;

// Use `pub use` to "lift" the wifi_create function up to the `crate::wifi` path.
pub use wifi::wifi_create;