// src/stepper/mod.rs

// 1. Declare the sub-modules within the `stepper` module.
//    The `pub` keyword makes them accessible to other parts of your crate
//    if needed, but we will create a cleaner public API below.
mod sr74hc595;
mod i2s0;
mod bitbang_driver;
mod spi_driver;
mod planner;
//mod drivetrain;

// 2. Re-export the primary types to create a clean public API.
//    This allows other files to write `use stepper::MotionPlanner;`
//    instead of the more verbose `use stepper::planner::MotionPlanner;`.
pub use i2s0::I2sStepperDriver;
pub use bitbang_driver::BitbangStepperDriver;
pub use spi_driver::SpiStepperDriver;

// pub use drivetrain::{TelescopeDrive, AxisConfig};
pub use planner::MotionPlanner;
pub use planner::PlannerMode;

// You could also re-export the constants if you need them externally.
// pub use sr74hc595::*; 

