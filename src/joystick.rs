use esp_hal::{
  Blocking,
  analog::adc::{Adc, AdcCalCurve, AdcChannel, AdcConfig, AdcPin, Attenuation},
  gpio::AnalogPin,
  peripherals::ADC1,
};

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

/// Generic 3-Axis Joystick driver using ADC1 channels.
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
    }
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

  /// Reads both raw millivolts and calibrated normalized `[-1.0, 1.0]` values for X, Y, and Z.
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
}

