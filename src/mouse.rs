//! USB HID Mouse implementation for 3D navigation and look-around.

use crate::joystick::JoystickReading;

/// Standard USB HID Mouse Report Descriptor.
///
/// Features:
/// - 5 buttons (Left, Right, Middle, Back, Forward)
/// - X, Y relative movement (-127 to 127)
/// - Wheel relative movement (-127 to 127)
#[rustfmt::skip]
pub const MOUSE_REPORT_DESCRIPTOR: &[u8] = &[
  0x05, 0x01,        // Usage Page (Generic Desktop)
  0x09, 0x02,        // Usage (Mouse)
  0xA1, 0x01,        // Collection (Application)
  0x09, 0x01,        //   Usage (Pointer)
  0xA1, 0x00,        //   Collection (Physical)
  
  // 5 Buttons (Left, Right, Middle, Back, Forward)
  0x05, 0x09,        //     Usage Page (Button)
  0x19, 0x01,        //     Usage Minimum (Button 1)
  0x29, 0x05,        //     Usage Maximum (Button 5)
  0x15, 0x00,        //     Logical Minimum (0)
  0x25, 0x01,        //     Logical Maximum (1)
  0x75, 0x01,        //     Report Size (1 bit)
  0x95, 0x05,        //     Report Count (5 buttons)
  0x81, 0x02,        //     Input (Data, Variable, Absolute)
  
  // Padding (3 bits)
  0x75, 0x03,        //     Report Size (3 bits)
  0x95, 0x01,        //     Report Count (1)
  0x81, 0x01,        //     Input (Constant)
  
  // X, Y relative movement
  0x05, 0x01,        //     Usage Page (Generic Desktop)
  0x09, 0x30,        //     Usage (X)
  0x09, 0x31,        //     Usage (Y)
  0x15, 0x81,        //     Logical Minimum (-127)
  0x25, 0x7F,        //     Logical Maximum (127)
  0x75, 0x08,        //     Report Size (8 bits)
  0x95, 0x02,        //     Report Count (2 axes)
  0x81, 0x06,        //     Input (Data, Variable, Relative)
  
  // Wheel relative movement (vertical scroll / zoom)
  0x09, 0x38,        //     Usage (Wheel)
  0x15, 0x81,        //     Logical Minimum (-127)
  0x25, 0x7F,        //     Logical Maximum (127)
  0x75, 0x08,        //     Report Size (8 bits)
  0x95, 0x01,        //     Report Count (1 axis)
  0x81, 0x06,        //     Input (Data, Variable, Relative)
  
  0xC0,              //   End Collection
  0xC0,              // End Collection
];

/// 4-byte standard USB HID Mouse Report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C, packed)]
pub struct MouseReport {
  /// Button bitmask (bit 0 = Left, bit 1 = Right, bit 2 = Middle, bit 3 = Back, bit 4 = Forward).
  pub buttons: u8,
  /// Relative X movement (-127 to 127).
  pub x: i8,
  /// Relative Y movement (-127 to 127).
  pub y: i8,
  /// Relative Wheel movement (-127 to 127).
  pub wheel: i8,
}

impl MouseReport {
  /// Returns whether this report contains any active movement or button presses.
  #[inline]
  pub fn has_activity(&self) -> bool {
    self.buttons != 0 || self.x != 0 || self.y != 0 || self.wheel != 0
  }

  /// Converts joystick deflection to relative mouse movement for 3D look-around.
  ///
  /// - `joy.x`: Controls horizontal look (right = +X, left = -X)
  /// - `joy.y`: Controls vertical look (forward/up = -Y in mouse coordinates, down = +Y)
  /// - `joy.z`: Controls zoom / scroll wheel via an accumulator
  /// - `middle_click`: If true, sets the Middle Mouse Button (MMB) for CAD/Blender orbit
  pub fn from_joystick(
    joy: JoystickReading,
    middle_click: bool,
    wheel_accum: &mut f32,
  ) -> Self {
    // Non-linear response curve for fine control near center and fast turning at edges:
    // With 200Hz polling rate, max_speed = 14 gives up to 2800 pixels/second.
    const MAX_SPEED: f32 = 14.0;

    let dx = (joy.x.abs() * joy.x * MAX_SPEED).clamp(-127.0, 127.0) as i8;
    // Mouse coordinate standard: moving up on screen is negative Y
    let dy = (-joy.y.abs() * joy.y * MAX_SPEED).clamp(-127.0, 127.0) as i8;

    // Handle Z axis as smooth scroll wheel (zoom in/out)
    *wheel_accum += joy.z * 0.15;
    let wheel_step = (*wheel_accum as i8).clamp(-127, 127);
    if wheel_step != 0 {
      *wheel_accum -= wheel_step as f32;
    }

    let mut buttons = 0u8;
    if middle_click {
      buttons |= 1 << 2; // Middle Mouse Button (bit 2)
    }

    Self {
      buttons,
      x: dx,
      y: dy,
      wheel: wheel_step,
    }
  }

  /// Serializes the report into 4 bytes for USB HID transmission.
  #[inline]
  pub fn to_bytes(&self) -> [u8; 4] {
    [
      self.buttons,
      self.x as u8,
      self.y as u8,
      self.wheel as u8,
    ]
  }
}
