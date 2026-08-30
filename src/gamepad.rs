use crate::joystick::{ActiveJoystick, JoystickReading};

/// Standard USB HID Gamepad Report Descriptor.
///
/// Features:
/// - 16 Generic Buttons (Buttons 1 to 16)
/// - Joystick 1: X, Y, Z axes (signed 8-bit, -127 to 127)
/// - Joystick 2: Rx, Ry, Rz axes (signed 8-bit, -127 to 127)
#[rustfmt::skip]
pub const GAMEPAD_REPORT_DESCRIPTOR: &[u8] = &[
  0x05, 0x01,        // Usage Page (Generic Desktop)
  0x09, 0x05,        // Usage (Game Pad)
  0xA1, 0x01,        // Collection (Application)
  
  // 16 Buttons (Buttons 1..16)
  0x05, 0x09,        //   Usage Page (Button)
  0x19, 0x01,        //   Usage Minimum (Button 1)
  0x29, 0x10,        //   Usage Maximum (Button 16)
  0x15, 0x00,        //   Logical Minimum (0)
  0x25, 0x01,        //   Logical Maximum (1)
  0x75, 0x01,        //   Report Size (1 bit)
  0x95, 0x10,        //   Report Count (16 buttons)
  0x81, 0x02,        //   Input (Data, Var, Abs)
  
  // Joystick 1: X, Y, Z axes
  0x05, 0x01,        //   Usage Page (Generic Desktop)
  0x09, 0x30,        //   Usage (X)
  0x09, 0x31,        //   Usage (Y)
  0x09, 0x32,        //   Usage (Z)
  0x15, 0x81,        //   Logical Minimum (-127)
  0x25, 0x7F,        //   Logical Maximum (127)
  0x75, 0x08,        //   Report Size (8 bits)
  0x95, 0x03,        //   Report Count (3 axes)
  0x81, 0x02,        //   Input (Data, Var, Abs)
  
  // Joystick 2: Rx, Ry, Rz axes
  0x05, 0x01,        //   Usage Page (Generic Desktop)
  0x09, 0x33,        //   Usage (Rx)
  0x09, 0x34,        //   Usage (Ry)
  0x09, 0x35,        //   Usage (Rz)
  0x15, 0x81,        //   Logical Minimum (-127)
  0x25, 0x7F,        //   Logical Maximum (127)
  0x75, 0x08,        //   Report Size (8 bits)
  0x95, 0x03,        //   Report Count (3 axes)
  0x81, 0x02,        //   Input (Data, Var, Abs)
  
  0xC0,              // End Collection
];

/// 8-byte USB Gamepad Report structure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C, packed)]
pub struct GamepadReport {
  /// 16 buttons bitmask (Bit 0 = Button 1, Bit 15 = Button 16).
  pub buttons: u16,
  /// Joystick 1 X axis (-127 to 127).
  pub x: i8,
  /// Joystick 1 Y axis (-127 to 127).
  pub y: i8,
  /// Joystick 1 Z axis (-127 to 127).
  pub z: i8,
  /// Joystick 2 X axis (-127 to 127).
  pub rx: i8,
  /// Joystick 2 Y axis (-127 to 127).
  pub ry: i8,
  /// Joystick 2 Z axis (-127 to 127).
  pub rz: i8,
}

impl GamepadReport {
  /// Converts a float in range [-1.0, 1.0] to a signed 8-bit integer [-127, 127].
  #[inline]
  pub fn float_to_i8(val: f32) -> i8 {
    (val * 127.0).clamp(-127.0, 127.0) as i8
  }

  /// Builds a GamepadReport from the current active joystick mode, physical readings, and 16 buttons.
  pub fn new(active: ActiveJoystick, joy: JoystickReading, buttons_bitmask: u16) -> Self {
    let axis_x = Self::float_to_i8(joy.x);
    let axis_y = Self::float_to_i8(joy.y);
    let axis_z = Self::float_to_i8(joy.z);

    match active {
      ActiveJoystick::Joystick1 => Self {
        buttons: buttons_bitmask,
        x: axis_x,
        y: axis_y,
        z: axis_z,
        rx: 0,
        ry: 0,
        rz: 0,
      },
      ActiveJoystick::Joystick2 => Self {
        buttons: buttons_bitmask,
        x: 0,
        y: 0,
        z: 0,
        rx: axis_x,
        ry: axis_y,
        rz: axis_z,
      },
    }
  }

  /// Serializes the report into raw 8 bytes for USB transmission.
  #[inline]
  pub fn to_bytes(&self) -> [u8; 8] {
    let btn_bytes = self.buttons.to_le_bytes();
    [
      btn_bytes[0],
      btn_bytes[1],
      self.x as u8,
      self.y as u8,
      self.z as u8,
      self.rx as u8,
      self.ry as u8,
      self.rz as u8,
    ]
  }
}
