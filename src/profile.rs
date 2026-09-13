//! Profile management for switching between FarmSim (Joystick/Gamepad) and Macropad (Keyboard/3D Mouse).

/// Active controller operating profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControllerProfile {
  /// Default FarmSim profile (HID Joystick with 2 virtual modes and 16 buttons).
  #[default]
  FarmSim,
  /// Macropad & 3D profile (F13..F24 keys and relative mouse look-around).
  Macropad,
}

impl ControllerProfile {
  /// Toggles between FarmSim and Macropad profiles.
  pub fn toggle(&mut self) -> Self {
    *self = match self {
      ControllerProfile::FarmSim => ControllerProfile::Macropad,
      ControllerProfile::Macropad => ControllerProfile::FarmSim,
    };
    *self
  }

  /// Returns a human-readable name of the profile.
  pub fn label(&self) -> &'static str {
    match self {
      ControllerProfile::FarmSim => "FARMSIM (Joystick & Buttons)",
      ControllerProfile::Macropad => "MACROPAD (F13-F24 & 3D Look)",
    }
  }
}
