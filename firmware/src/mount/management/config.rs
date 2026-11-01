// src/mount/management/config.rs

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct TelescopeConfig {
    pub motorx_spr: u32,
    pub motorx_microsteps: u32,
    pub motorx_gratio: f64,
    pub motory_spr: u32,
    pub motory_microsteps: u32,
    pub motory_gratio: f64,
    pub mount_type: String,
    pub ra_reversed: bool,
    pub dec_reversed: bool,
}

#[derive(Deserialize, Debug)]
pub struct FocuserConfig {
    pub motorz_spr: u32,
    pub motorz_microsteps: u32,
    pub motorz_gratio: f64,
    pub focuser_max_step: u32,
    pub focuser_step_size: f64,
    pub focuser_reversed: bool,
}
