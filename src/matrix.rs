use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull};
use heapless::Vec;

/// Standard 4x4 keypad key mapping.
pub const DEFAULT_KEY_MAP: [[char; 4]; 4] = [
  ['1', '2', '3', 'A'], // Row 0 (Pin 5)
  ['4', '5', '6', 'B'], // Row 1 (Pin 6)
  ['7', '8', '9', 'C'], // Row 2 (Pin 7)
  ['*', '0', '#', 'D'], // Row 3 (Pin 8)
];

/// A key event representing a key press or release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
  Pressed {
    row: u8,
    col: u8,
    key: char,
    index: u8,
  },
  Released {
    row: u8,
    col: u8,
    key: char,
    index: u8,
  },
}

/// Diode orientation in the matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiodeDirection {
  /// Cathode points towards the Row (`Column -> Row`). Standard for most custom/QMK keyboard matrices.
  /// Columns use internal `Pull::Up` (idle HIGH), rows are driven `LOW`, key press pulls column `LOW`.
  ColToRow,
  /// Cathode points towards the Column (`Row -> Column`).
  /// Columns use internal `Pull::Down` (idle LOW), rows are driven `HIGH`, key press pulls column `HIGH`.
  RowToCol,
}

/// 4x4 Matrix Keypad driver supporting anti-ghosting diodes and software debouncing.
pub struct MatrixKeypad4x4<'a> {
  rows: [Output<'a>; 4],
  cols: [Input<'a>; 4],
  pub keymap: [[char; 4]; 4],
  pub diode_direction: DiodeDirection,
  last_raw_state: u16,
  debounced_state: u16,
  debounce_counters: [u8; 16],
  debounce_threshold: u8,
}

impl<'a> MatrixKeypad4x4<'a> {
  /// Creates a new `MatrixKeypad4x4` instance from 4 Row outputs and 4 Column inputs.
  /// Defaults to `DiodeDirection::ColToRow` (Active-Low with internal Pull-Up on columns).
  ///
  /// - `rows`: Pins 5, 6, 7, 8
  /// - `cols`: Pins 9, 10, 11, 12
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    row0: impl OutputPin + 'a,
    row1: impl OutputPin + 'a,
    row2: impl OutputPin + 'a,
    row3: impl OutputPin + 'a,
    col0: impl InputPin + 'a,
    col1: impl InputPin + 'a,
    col2: impl InputPin + 'a,
    col3: impl InputPin + 'a,
  ) -> Self {
    Self::with_diode_direction(
      row0,
      row1,
      row2,
      row3,
      col0,
      col1,
      col2,
      col3,
      DiodeDirection::ColToRow,
    )
  }

  /// Creates a new `MatrixKeypad4x4` instance specifying the diode direction.
  #[allow(clippy::too_many_arguments)]
  pub fn with_diode_direction(
    row0: impl OutputPin + 'a,
    row1: impl OutputPin + 'a,
    row2: impl OutputPin + 'a,
    row3: impl OutputPin + 'a,
    col0: impl InputPin + 'a,
    col1: impl InputPin + 'a,
    col2: impl InputPin + 'a,
    col3: impl InputPin + 'a,
    diode_direction: DiodeDirection,
  ) -> Self {
    let out_cfg = OutputConfig::default();
    let (idle_row_level, pull) = match diode_direction {
      DiodeDirection::ColToRow => (Level::High, Pull::Up),
      DiodeDirection::RowToCol => (Level::Low, Pull::Down),
    };
    let in_cfg = InputConfig::default().with_pull(pull);

    let rows = [
      Output::new(row0, idle_row_level, out_cfg),
      Output::new(row1, idle_row_level, out_cfg),
      Output::new(row2, idle_row_level, out_cfg),
      Output::new(row3, idle_row_level, out_cfg),
    ];

    let cols = [
      Input::new(col0, in_cfg),
      Input::new(col1, in_cfg),
      Input::new(col2, in_cfg),
      Input::new(col3, in_cfg),
    ];

    Self {
      rows,
      cols,
      keymap: DEFAULT_KEY_MAP,
      diode_direction,
      last_raw_state: 0,
      debounced_state: 0,
      debounce_counters: [0; 16],
      debounce_threshold: 2, // 2 matching consecutive scans required
    }
  }

  /// Sets a custom debounce threshold (number of consecutive stable scans, default is 2).
  pub fn set_debounce_threshold(&mut self, threshold: u8) {
    self.debounce_threshold = threshold.max(1);
  }

  /// Sets a custom character key mapping for the 4x4 matrix.
  pub fn set_keymap(&mut self, keymap: [[char; 4]; 4]) {
    self.keymap = keymap;
  }

  /// Performs a single hardware scan of the 4x4 matrix.
  /// Returns a 16-bit bitmask where bit `(row * 4 + col)` is 1 if pressed, 0 if released.
  pub fn scan_raw(&mut self) -> u16 {
    let mut state: u16 = 0;

    match self.diode_direction {
      DiodeDirection::ColToRow => {
        // Active-Low scan: drive active row LOW, read column LOW
        for r in 0..4 {
          self.rows[r].set_level(Level::Low);

          for _ in 0..100 {
            core::hint::spin_loop();
          }

          for c in 0..4 {
            if self.cols[c].is_low() {
              state |= 1 << (r * 4 + c);
            }
          }

          self.rows[r].set_level(Level::High);
        }
      }
      DiodeDirection::RowToCol => {
        // Active-High scan: drive active row HIGH, read column HIGH
        for r in 0..4 {
          self.rows[r].set_level(Level::High);

          for _ in 0..100 {
            core::hint::spin_loop();
          }

          for c in 0..4 {
            if self.cols[c].is_high() {
              state |= 1 << (r * 4 + c);
            }
          }

          self.rows[r].set_level(Level::Low);
        }
      }
    }

    state
  }


  /// Scans the matrix, applies debounce filtering, and returns any new KeyEvents (Pressed/Released).
  pub fn update(&mut self) -> Vec<KeyEvent, 16> {
    let raw = self.scan_raw();
    let mut events = Vec::new();

    for index in 0..16 {
      let is_raw_pressed = (raw & (1 << index)) != 0;
      let is_currently_debounced = (self.debounced_state & (1 << index)) != 0;

      if is_raw_pressed != is_currently_debounced {
        self.debounce_counters[index] = self.debounce_counters[index].saturating_add(1);
        if self.debounce_counters[index] >= self.debounce_threshold {
          self.debounce_counters[index] = 0;
          let row = (index / 4) as u8;
          let col = (index % 4) as u8;
          let key = self.keymap[row as usize][col as usize];

          if is_raw_pressed {
            self.debounced_state |= 1 << index;
            let _ = events.push(KeyEvent::Pressed {
              row,
              col,
              key,
              index: index as u8,
            });
          } else {
            self.debounced_state &= !(1 << index);
            let _ = events.push(KeyEvent::Released {
              row,
              col,
              key,
              index: index as u8,
            });
          }
        }
      } else {
        self.debounce_counters[index] = 0;
      }
    }

    self.last_raw_state = raw;
    events
  }

  /// Checks if a key at the specified row (0..3) and col (0..3) is currently held down.
  pub fn is_pressed(&self, row: usize, col: usize) -> bool {
    if row < 4 && col < 4 {
      (self.debounced_state & (1 << (row * 4 + col))) != 0
    } else {
      false
    }
  }

  /// Checks if a key matching the given character label is currently held down.
  pub fn is_key_pressed(&self, key: char) -> bool {
    for r in 0..4 {
      for c in 0..4 {
        if self.keymap[r][c] == key {
          return self.is_pressed(r, c);
        }
      }
    }
    false
  }

  /// Returns the current 16-bit debounced state bitmask.
  pub fn debounced_state(&self) -> u16 {
    self.debounced_state
  }
}
