// src/stepper/sr74hc595.rs

// Bit positions in the shift register (74HC595)
// Q0 - XYZEnable (active low) - bit 0
// Q1 - X_Step   - bit 1
// Q2 - X_dir    - bit 2
// Q3 - Z_step   - bit 3
// Q4 - Z_dir    - bit 4
// Q5 - Y_step   - bit 5
// Q6 - Y_dir    - bit 6
// Q7 - BEEPER   - bit 7



/// Shift Register Output Mapping (Q7..Q0)
pub const BEEPER_PIN: u8   = 1 << 7;  // 0b10000000
pub const Y_DIR_PIN: u8    = 1 << 6;  // 0b01000000
pub const Y_STEP_PIN: u8   = 1 << 5;  // 0b00100000
pub const Z_DIR_PIN: u8    = 1 << 4;  // 0b00010000
pub const Z_STEP_PIN: u8   = 1 << 3;  // 0b00001000
pub const X_DIR_PIN: u8    = 1 << 2;  // 0b00000100
pub const X_STEP_PIN: u8   = 1 << 1;  // 0b00000010
pub const ENABLE_PIN: u8   = 1 << 0;  // 0b00000001  - Active Low


/// Represents the 8-bit state of the 74HC595 shift register.
/// Provides a safe, chainable API to manipulate individual pins without 
/// bitwise operations cluttering the motion planner.
#[derive(Clone, Copy, Debug, Default)]
pub struct MotorState(pub u8);

impl MotorState {
    /// Creates a new MotorState with all pins low (motors disabled by default).
    pub fn new() -> Self {
        Self(0)
    }

    /// Enables or disables the stepper drivers.
    /// Note: ENABLE_PIN is Active Low, so `enable=true` clears the bit.
    pub fn enable_motors(self, enable: bool) -> Self {
        if enable { Self(self.0 & !ENABLE_PIN) } else { Self(self.0 | ENABLE_PIN) }
    }
    
    pub fn set_x_dir(self, positive: bool) -> Self {
        if positive { Self(self.0 | X_DIR_PIN) } else { Self(self.0 & !X_DIR_PIN) }
    }
    
    pub fn set_y_dir(self, positive: bool) -> Self {
        if positive { Self(self.0 | Y_DIR_PIN) } else { Self(self.0 & !Y_DIR_PIN) }
    }

    pub fn set_z_dir(self, positive: bool) -> Self {
        if positive { Self(self.0 | Z_DIR_PIN) } else { Self(self.0 & !Z_DIR_PIN) }
    }
    
    /// Sets the X step bit HIGH. 
    /// CRITICAL: Does NOT clear other step bits, allowing simultaneous multi-axis stepping.
    pub fn step_x(self) -> Self {
        Self(self.0 | X_STEP_PIN)
    }

    /// Sets the Y step bit HIGH.
    /// CRITICAL: Does NOT clear other step bits, allowing simultaneous multi-axis stepping.
    pub fn step_y(self) -> Self {
        Self(self.0 | Y_STEP_PIN)
    }

    /// Sets the Z step bit HIGH.
    /// CRITICAL: Does NOT clear other step bits, allowing simultaneous multi-axis stepping.
    pub fn step_z(self) -> Self {
        Self(self.0 | Z_STEP_PIN)
    }
    
    /// Clears all step bits (X, Y, Z) to end a step pulse.
    pub fn step_low(self) -> Self {
        Self(self.0 & !(X_STEP_PIN | Y_STEP_PIN | Z_STEP_PIN))
    }

    pub fn beeper_on(self) -> Self { Self(self.0 | BEEPER_PIN) }
    /// Turns off the beeper by clearing the BEEPER_PIN bit.
    pub fn beeper_off(self) -> Self { Self(self.0 & !BEEPER_PIN) }
    
}
