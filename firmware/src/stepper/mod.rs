// src/stepper/mod.rs


mod sr74hc595;
mod i2s0;  // needs attention - not working
mod bitbang_driver;  // <- using this driver for now
mod spi_driver;  // needs attention - not working
mod planner;



pub use i2s0::I2sStepperDriver;
pub use bitbang_driver::BitbangStepperDriver;
pub use spi_driver::SpiStepperDriver;


pub use planner::MotionPlanner;
pub use planner::PlannerMode;

