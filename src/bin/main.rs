#![no_std]
#![no_main]
#![deny(
  clippy::mem_forget,
  reason = "mem::forget is generally not safe to do with esp_hal types holding buffers."
)]
#![deny(clippy::large_stack_frames)]

extern crate alloc;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embassy_usb::{Builder, Config as UsbConfig, UsbDevice};
use embassy_usb::class::hid::{Config as HidConfig, HidWriter, State as HidState};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::otg_fs::{asynch::Config as OtgConfig, asynch::Driver, Usb};
use esp_hal::peripherals::Peripherals;
use esp_hal::timer::timg::{MwdtStage, TimerGroup};
use esp32s3_farmstick::{
  gamepad::{GamepadReport, GAMEPAD_REPORT_DESCRIPTOR},
  joystick::{DebouncedButton, Joystick},
  matrix::MatrixKeypad4x4,
  mk_static,
};

// App descriptor required by esp-idf bootloader
esp_bootloader_esp_idf::esp_app_desc!();

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static>>) {
  usb.run().await;
}

#[allow(
  clippy::large_stack_frames,
  reason = "Main entry point allocates static USB buffers and driver instances"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
  let peripherals = init();

  // Initialize Embassy RTOS scheduler with TimerGroup 0
  let timg0 = TimerGroup::new(peripherals.TIMG0);
  let mut wdt0 = timg0.wdt;
  let sw_interrupt =
    esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
  esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

  // Main Watchdog Timer (MWDT) setup
  wdt0.enable();
  wdt0.set_timeout(MwdtStage::Stage0, esp_hal::time::Duration::from_secs(30));

  // Initialize 3-axis Joystick on ADC1 (X=GPIO1, Y=GPIO2, Z=GPIO4)
  let mut joystick = Joystick::new(
    peripherals.ADC1,
    peripherals.GPIO1,
    peripherals.GPIO2,
    peripherals.GPIO4,
  );

  // Calibrate joystick center in rest mode (64 samples)
  joystick.calibrate_center(64);

  // Initialize Mode Switch Button on GPIO 13 (with internal Pull-Up to GND)
  let mut switch_button = DebouncedButton::new_pullup(peripherals.GPIO13);

  // Initialize 4x4 Matrix Keypad (Rows: 5,6,7,8 | Cols: 9,10,11,12)
  let mut keypad = MatrixKeypad4x4::new(
    peripherals.GPIO5,
    peripherals.GPIO6,
    peripherals.GPIO7,
    peripherals.GPIO8,
    peripherals.GPIO9,
    peripherals.GPIO10,
    peripherals.GPIO11,
    peripherals.GPIO12,
  );

  // Static buffers for USB OTG and HID Gamepad
  let ep_out_buffer = mk_static!([u8; 256], [0u8; 256]);
  let config_descriptor = mk_static!([u8; 256], [0u8; 256]);
  let bos_descriptor = mk_static!([u8; 256], [0u8; 256]);
  let msos_descriptor = mk_static!([u8; 256], [0u8; 256]);
  let control_buf = mk_static!([u8; 64], [0u8; 64]);
  let hid_state = mk_static!(HidState, HidState::new());

  // Initialize USB OTG peripheral on native USB pins (DP=GPIO20, DM=GPIO19)
  let usb_peri = Usb::new(peripherals.USB0, peripherals.GPIO20, peripherals.GPIO19);
  let usb_driver = Driver::new(usb_peri, ep_out_buffer, OtgConfig::default());

  // USB Device configuration (Generic Gamepad)
  let mut usb_config = UsbConfig::new(0x1209, 0x2001);
  usb_config.manufacturer = Some("FarmStick");
  usb_config.product = Some("FarmStick Gamepad Controller");
  usb_config.serial_number = Some("FS-0001");
  usb_config.max_power = 100;
  usb_config.max_packet_size_0 = 64;

  let mut builder = Builder::new(
    usb_driver,
    usb_config,
    config_descriptor,
    bos_descriptor,
    msos_descriptor,
    control_buf,
  );

  // USB HID Gamepad configuration (200Hz polling rate)
  let hid_config = HidConfig {
    report_descriptor: GAMEPAD_REPORT_DESCRIPTOR,
    request_handler: None,
    poll_ms: 5,
    max_packet_size: 64,
  };

  let mut writer = HidWriter::<_, 8>::new(&mut builder, hid_state, hid_config);
  let usb_device = builder.build();

  // Spawn background USB device task
  spawner.spawn(usb_task(usb_device).unwrap());

  // Wait until USB endpoint is enabled by host
  writer.ready().await;

  loop {
    // Feed watchdog timer regularly
    wdt0.feed();

    // 1. Check for Joystick Switch Button press on Pin 13 (toggles J1 <-> J2)
    if switch_button.update_just_pressed() {
      joystick.toggle_mode();
    }

    // 2. Scan 4x4 matrix keypad to update debounced key states (16 buttons)
    let _ = keypad.update();
    let buttons_bitmask = keypad.debounced_state();

    // 3. Read physical joystick
    let joy_reading = joystick.read();

    // 4. Build HID Gamepad report
    let report = GamepadReport::new(joystick.active_mode(), joy_reading, buttons_bitmask);

    // 5. Send report over USB HID
    let _ = writer.write(&report.to_bytes()).await;

    // 5ms interval = 200 Hz update rate
    Timer::after(Duration::from_millis(5)).await;
  }
}

/// Initializes CPU clock and 64KB heap allocator.
fn init() -> Peripherals {
  let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
  let peripherals = esp_hal::init(config);

  // Initialize heap allocator (reclaiming unused RAM)
  esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);

  peripherals
}

