use esp_hal::{
  Blocking,
  analog::adc::{Adc, AdcCalCurve, AdcChannel, AdcConfig, AdcPin, Attenuation},
  gpio::AnalogPin,
  peripherals::ADC1,
};

/// Active virtual joystick selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveJoystick {
  #[default]
  Joystick1,
  Joystick2,
}

impl ActiveJoystick {
  /// Toggles between Virtual Joystick 1 and Virtual Joystick 2.
  pub fn toggle(&mut self) -> Self {
    *self = match self {
      ActiveJoystick::Joystick1 => ActiveJoystick::Joystick2,
      ActiveJoystick::Joystick2 => ActiveJoystick::Joystick1,
    };
    *self
  }

  /// Returns the human-readable label for the active joystick.
  pub fn label(&self) -> &'static str {
    match self {
      ActiveJoystick::Joystick1 => "JOYSTICK 1 (Primary)",
      ActiveJoystick::Joystick2 => "JOYSTICK 2 (Secondary)",
    }
  }
}

/// Dual virtual joystick readings (one active receives physical input, inactive remains centered).
#[derive(Debug, Clone, Copy, Default)]
pub struct DualJoystickReading {
  /// Currently active virtual joystick mode.
  pub active: ActiveJoystick,
  /// Current values for Virtual Joystick 1.
  pub joy1: JoystickReading,
  /// Current values for Virtual Joystick 2.
  pub joy2: JoystickReading,
}

/// 3-Axis Joystick measurement holding calibrated millivolts and normalized -1.0..1.0 values.
#[derive(Debug, Clone, Copy, Default)]
pub struct JoystickReading {
  /// Raw calibrated millivolts measured on the X-axis ADC pin.
  pub raw_x_mv: u16,
  /// Raw calibrated millivolts measured on the Y-axis ADC pin.
  pub raw_y_mv: u16,
  /// Raw calibrated millivolts measured on the Z-axis ADC pin.
  pub raw_z_mv: u16,
  /// Normalized X-axis position (-1.0 to 1.0, 0.0 is center).
  pub x: f32,
  /// Normalized Y-axis position (-1.0 to 1.0, 0.0 is center).
  pub y: f32,
  /// Normalized Z-axis position (-1.0 to 1.0, 0.0 is center).
  pub z: f32,
}

/// Calibration parameters for each joystick axis.
#[derive(Debug, Clone, Copy)]
pub struct AxisConfig {
  pub min_mv: u16,
  pub center_mv: u16,
  pub max_mv: u16,
  pub deadzone_mv: u16,
  pub invert: bool,
}

impl Default for AxisConfig {
  fn default() -> Self {
    Self {
      min_mv: 0,
      center_mv: 1550, // Typical half-rail for 3.3V with 11dB attenuation (~3100mV full scale)
      max_mv: 3100,
      deadzone_mv: 80,
      invert: false,
    }
  }
}

impl AxisConfig {
  /// Converts a raw millivolt reading to a normalized float in the range `[-1.0, 1.0]`.
  /// Seamlessly scales from 0.0 at the deadzone edge up to ±1.0.
  pub fn normalize(&self, raw_mv: u16) -> f32 {
    let diff = raw_mv as i32 - self.center_mv as i32;
    if diff.abs() <= self.deadzone_mv as i32 {
      return 0.0;
    }

    let val = if diff > 0 {
      let active_diff = diff - self.deadzone_mv as i32;
      let active_span =
        (self.max_mv as i32 - self.center_mv as i32 - self.deadzone_mv as i32).max(1);
      active_diff as f32 / active_span as f32
    } else {
      let active_diff = diff + self.deadzone_mv as i32;
      let active_span =
        (self.center_mv as i32 - self.min_mv as i32 - self.deadzone_mv as i32).max(1);
      active_diff as f32 / active_span as f32
    };

    let clamped = val.clamp(-1.0, 1.0);
    if self.invert { -clamped } else { clamped }
  }
}

/// Complete configuration for a 3-axis joystick.
#[derive(Debug, Clone, Copy, Default)]
pub struct JoystickConfig {
  pub x: AxisConfig,
  pub y: AxisConfig,
  pub z: AxisConfig,
}

/// Generic 3-Axis Joystick driver using ADC1 channels with virtual mode switching.
pub struct Joystick<'a, PX, PY, PZ>
where
  PX: AnalogPin + AdcChannel,
  PY: AnalogPin + AdcChannel,
  PZ: AnalogPin + AdcChannel,
{
  adc: Adc<'a, ADC1<'a>, Blocking>,
  pin_x: AdcPin<PX, ADC1<'a>, AdcCalCurve<ADC1<'a>>>,
  pin_y: AdcPin<PY, ADC1<'a>, AdcCalCurve<ADC1<'a>>>,
  pin_z: AdcPin<PZ, ADC1<'a>, AdcCalCurve<ADC1<'a>>>,
  pub config: JoystickConfig,
  pub active_mode: ActiveJoystick,
}

impl<'a, PX, PY, PZ> Joystick<'a, PX, PY, PZ>
where
  PX: AnalogPin + AdcChannel,
  PY: AnalogPin + AdcChannel,
  PZ: AnalogPin + AdcChannel,
{
  /// Creates a new `Joystick` instance on ADC1 with the specified pins for X, Y, and Z axes.
  pub fn new(
    adc_instance: ADC1<'a>,
    pin_x: PX,
    pin_y: PY,
    pin_z: PZ,
  ) -> Self {
    Self::with_config(adc_instance, pin_x, pin_y, pin_z, JoystickConfig::default())
  }

  /// Creates a new `Joystick` instance with custom axis configurations.
  pub fn with_config(
    adc_instance: ADC1<'a>,
    pin_x: PX,
    pin_y: PY,
    pin_z: PZ,
    config: JoystickConfig,
  ) -> Self {
    let mut adc1_config = AdcConfig::new();

    // Enable pins with factory eFuse curve calibration for optimal voltage precision
    let cal_x =
      adc1_config.enable_pin_with_cal::<PX, AdcCalCurve<ADC1<'a>>>(pin_x, Attenuation::_11dB);
    let cal_y =
      adc1_config.enable_pin_with_cal::<PY, AdcCalCurve<ADC1<'a>>>(pin_y, Attenuation::_11dB);
    let cal_z =
      adc1_config.enable_pin_with_cal::<PZ, AdcCalCurve<ADC1<'a>>>(pin_z, Attenuation::_11dB);

    let adc = Adc::new(adc_instance, adc1_config);

    Self {
      adc,
      pin_x: cal_x,
      pin_y: cal_y,
      pin_z: cal_z,
      config,
      active_mode: ActiveJoystick::default(),
    }
  }

  /// Toggles between Virtual Joystick 1 and Virtual Joystick 2.
  pub fn toggle_mode(&mut self) -> ActiveJoystick {
    self.active_mode.toggle()
  }

  /// Sets the active virtual joystick mode.
  pub fn set_active_mode(&mut self, mode: ActiveJoystick) {
    self.active_mode = mode;
  }

  /// Returns the currently active virtual joystick mode.
  pub fn active_mode(&self) -> ActiveJoystick {
    self.active_mode
  }

  /// Calibrates the center zero-position by averaging multiple samples at rest.
  pub fn calibrate_center(&mut self, samples: u32) {
    let count = samples.max(1);
    let mut sum_x: u32 = 0;
    let mut sum_y: u32 = 0;
    let mut sum_z: u32 = 0;

    for _ in 0..count {
      let (x, y, z) = self.read_raw_mv();
      sum_x += x as u32;
      sum_y += y as u32;
      sum_z += z as u32;
    }

    self.config.x.center_mv = (sum_x / count) as u16;
    self.config.y.center_mv = (sum_y / count) as u16;
    self.config.z.center_mv = (sum_z / count) as u16;
  }

  /// Reads raw calibrated millivolts for all three axes `(x, y, z)`.
  pub fn read_raw_mv(&mut self) -> (u16, u16, u16) {
    let x_mv = nb::block!(self.adc.read_oneshot(&mut self.pin_x)).unwrap_or(0);
    let y_mv = nb::block!(self.adc.read_oneshot(&mut self.pin_y)).unwrap_or(0);
    let z_mv = nb::block!(self.adc.read_oneshot(&mut self.pin_z)).unwrap_or(0);
    (x_mv, y_mv, z_mv)
  }

  /// Reads raw millivolts oversampled and averaged across `samples` passes.
  pub fn read_raw_mv_averaged(&mut self, samples: u8) -> (u16, u16, u16) {
    let count = samples.max(1) as u32;
    let mut sum_x: u32 = 0;
    let mut sum_y: u32 = 0;
    let mut sum_z: u32 = 0;

    for _ in 0..count {
      let (x, y, z) = self.read_raw_mv();
      sum_x += x as u32;
      sum_y += y as u32;
      sum_z += z as u32;
    }

    (
      (sum_x / count) as u16,
      (sum_y / count) as u16,
      (sum_z / count) as u16,
    )
  }

  /// Reads physical joystick values (raw millivolts and normalized `[-1.0, 1.0]`).
  pub fn read(&mut self) -> JoystickReading {
    let (raw_x, raw_y, raw_z) = self.read_raw_mv_averaged(4);

    JoystickReading {
      raw_x_mv: raw_x,
      raw_y_mv: raw_y,
      raw_z_mv: raw_z,
      x: self.config.x.normalize(raw_x),
      y: self.config.y.normalize(raw_y),
      z: self.config.z.normalize(raw_z),
    }
  }

  /// Reads dual virtual joysticks: routes physical movement to the active joystick,
  /// while keeping the inactive joystick at neutral center (0.0).
  pub fn read_dual(&mut self) -> DualJoystickReading {
    let physical = self.read();
    let neutral = JoystickReading {
      raw_x_mv: self.config.x.center_mv,
      raw_y_mv: self.config.y.center_mv,
      raw_z_mv: self.config.z.center_mv,
      x: 0.0,
      y: 0.0,
      z: 0.0,
    };

    match self.active_mode {
      ActiveJoystick::Joystick1 => DualJoystickReading {
        active: ActiveJoystick::Joystick1,
        joy1: physical,
        joy2: neutral,
      },
      ActiveJoystick::Joystick2 => DualJoystickReading {
        active: ActiveJoystick::Joystick2,
        joy1: neutral,
        joy2: physical,
      },
    }
  }
}

/// Helper for debouncing a push button on a GPIO input pin.
pub struct DebouncedButton<'a> {
  pin: esp_hal::gpio::Input<'a>,
  is_pressed: bool,
  counter: u8,
  threshold: u8,
  active_low: bool,
}

impl<'a> DebouncedButton<'a> {
  /// Creates a new active-low button with internal Pull-Up (e.g. Button to GND).
  pub fn new_pullup(pin: impl esp_hal::gpio::InputPin + 'a) -> Self {
    let in_cfg = esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up);
    Self {
      pin: esp_hal::gpio::Input::new(pin, in_cfg),
      is_pressed: false,
      counter: 0,
      threshold: 2, // 2 consecutive scans required
      active_low: true,
    }
  }

  /// Creates a new active-high button with internal Pull-Down (e.g. Button to 3.3V).
  pub fn new_pulldown(pin: impl esp_hal::gpio::InputPin + 'a) -> Self {
    let in_cfg = esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Down);
    Self {
      pin: esp_hal::gpio::Input::new(pin, in_cfg),
      is_pressed: false,
      counter: 0,
      threshold: 2,
      active_low: false,
    }
  }

  /// Scans the button and returns `true` on the rising edge of a button press (just pressed).
  pub fn update_just_pressed(&mut self) -> bool {
    let raw_pressed = if self.active_low {
      self.pin.is_low()
    } else {
      self.pin.is_high()
    };

    if raw_pressed != self.is_pressed {
      self.counter = self.counter.saturating_add(1);
      if self.counter >= self.threshold {
        self.counter = 0;
        self.is_pressed = raw_pressed;
        if self.is_pressed {
          return true;
        }
      }
    } else {
      self.counter = 0;
    }

    false
  }

  /// Returns whether the button is currently held down.
  pub fn is_held(&self) -> bool {
    self.is_pressed
  }
}

