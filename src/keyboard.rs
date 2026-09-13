//! USB HID Keyboard implementation for Macropad mode.
//!
//! Provides a standard USB Boot Keyboard report descriptor and maps the 16 keypad
//! buttons to extended function keys (F13..F24) and additional control keys.

/// Standard USB HID Keyboard Report Descriptor (Boot Keyboard).
#[rustfmt::skip]
pub const KEYBOARD_REPORT_DESCRIPTOR: &[u8] = &[
  0x05, 0x01,        // Usage Page (Generic Desktop)
  0x09, 0x06,        // Usage (Keyboard)
  0xA1, 0x01,        // Collection (Application)
  
  // 8 Modifier bits (Left/Right Ctrl, Shift, Alt, GUI)
  0x05, 0x07,        //   Usage Page (Key Codes)
  0x19, 0xE0,        //   Usage Minimum (Keyboard Left Control)
  0x29, 0xE7,        //   Usage Maximum (Keyboard Right GUI)
  0x15, 0x00,        //   Logical Minimum (0)
  0x25, 0x01,        //   Logical Maximum (1)
  0x75, 0x01,        //   Report Size (1 bit)
  0x95, 0x08,        //   Report Count (8 bits)
  0x81, 0x02,        //   Input (Data, Variable, Absolute)
  
  // 1 Reserved byte
  0x95, 0x01,        //   Report Count (1 byte)
  0x75, 0x08,        //   Report Size (8 bits)
  0x81, 0x01,        //   Input (Constant)
  
  // 6 Keycode bytes (Array of up to 6 simultaneously pressed keys)
  0x95, 0x06,        //   Report Count (6 keys)
  0x75, 0x08,        //   Report Size (8 bits)
  0x15, 0x00,        //   Logical Minimum (0)
  0x25, 0xFF,        //   Logical Maximum (255)
  0x05, 0x07,        //   Usage Page (Key Codes)
  0x19, 0x00,        //   Usage Minimum (0)
  0x29, 0xFF,        //   Usage Maximum (255)
  0x81, 0x00,        //   Input (Data, Array)
  
  0xC0,              // End Collection
];

/// Mapping of the 16 keypad button indices to USB HID usage keycodes.
///
/// Buttons 1..12 map to F13..F24.
/// Buttons 13..16 map to unassigned/macro keys (Execute, Help, Menu, Select).
pub const MACROPAD_KEYCODES: [u8; 16] = [
  0x68, // Button 1  (index 0)  -> F13
  0x69, // Button 2  (index 1)  -> F14
  0x6A, // Button 3  (index 2)  -> F15
  0x6B, // Button 4  (index 3)  -> F16
  0x6C, // Button 5  (index 4)  -> F17
  0x6D, // Button 6  (index 5)  -> F18
  0x6E, // Button 7  (index 6)  -> F19
  0x6F, // Button 8  (index 7)  -> F20
  0x70, // Button 9  (index 8)  -> F21
  0x71, // Button 10 (index 9)  -> F22
  0x72, // Button 11 (index 10) -> F23
  0x73, // Button 12 (index 11) -> F24
  0x74, // Button 13 (index 12) -> Keyboard Execute
  0x75, // Button 14 (index 13) -> Keyboard Help
  0x76, // Button 15 (index 14) -> Keyboard Menu
  0x77, // Button 16 (index 15) -> Keyboard Select
];

/// 8-byte standard USB HID Keyboard Report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C, packed)]
pub struct KeyboardReport {
  /// Modifier keys (Ctrl, Shift, Alt, GUI).
  pub modifiers: u8,
  /// Reserved byte (always 0).
  pub reserved: u8,
  /// Up to 6 active keycodes (6KRO).
  pub keycodes: [u8; 6],
}

impl KeyboardReport {
  /// Creates a keyboard report from a 16-bit keypad button bitmask.
  ///
  /// Maps each pressed button bit to its corresponding F13..F24 keycode,
  /// populating up to 6 keys simultaneously.
  pub fn from_buttons_bitmask(buttons_bitmask: u16) -> Self {
    let mut keycodes = [0u8; 6];
    let mut count = 0;

    for (index, &keycode) in MACROPAD_KEYCODES.iter().enumerate() {
      if (buttons_bitmask & (1 << index)) != 0 && count < 6 {
        keycodes[count] = keycode;
        count += 1;
      }
    }

    Self {
      modifiers: 0,
      reserved: 0,
      keycodes,
    }
  }

  /// Returns whether any key is currently pressed.
  #[inline]
  pub fn has_pressed_keys(&self) -> bool {
    self.modifiers != 0 || self.keycodes.iter().any(|&k| k != 0)
  }

  /// Serializes the report into 8 bytes for USB HID transmission.
  #[inline]
  pub fn to_bytes(&self) -> [u8; 8] {
    [
      self.modifiers,
      self.reserved,
      self.keycodes[0],
      self.keycodes[1],
      self.keycodes[2],
      self.keycodes[3],
      self.keycodes[4],
      self.keycodes[5],
    ]
  }
}
