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
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::peripherals::Peripherals;
use esp_hal::timer::timg::{MwdtStage, TimerGroup};
use log::info;
use esp32s3_template::{
  joystick::{ActiveJoystick, DebouncedButton, Joystick},
  matrix::{KeyEvent, MatrixKeypad4x4},
};

// App descriptor required by esp-idf bootloader
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
  clippy::large_stack_frames,
  reason = "Main entry point may allocate initialization buffers"
)]
#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
  let peripherals = init();

  // Initialize Embassy RTOS scheduler with TimerGroup 0
  let timg0 = TimerGroup::new(peripherals.TIMG0);
  let mut wdt0 = timg0.wdt;
  let sw_interrupt =
    esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
  esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

  info!("ESP32-S3 Embassy Runtime initialized!");

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
  info!("Calibrating joystick center (keep joystick at rest)...");
  joystick.calibrate_center(64);
  info!(
    "Joystick calibrated! Centers: X={} mV, Y={} mV, Z={} mV",
    joystick.config.x.center_mv,
    joystick.config.y.center_mv,
    joystick.config.z.center_mv,
  );

  // Initialize Mode Switch Button on GPIO 13 (with internal Pull-Up to GND)
  let mut switch_button = DebouncedButton::new_pullup(peripherals.GPIO13);
  info!("Joystick Switch Button initialized on GPIO 13 (Press to toggle J1 <-> J2)");

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
  info!("4x4 Matrix Keypad initialized (Rows: 5, 6, 7, 8 | Cols: 9, 10, 11, 12)");

  let mut tick_counter: u32 = 0;

  loop {
    // Feed watchdog timer regularly
    wdt0.feed();

    // Check for Joystick Switch Button press on Pin 13
    if switch_button.update_just_pressed() {
      let new_mode = joystick.toggle_mode();
      info!(">>> MODE SWITCH: Active Joystick changed to {} <<<", new_mode.label());
    }

    // Scan matrix keypad and handle press/release events
    let key_events = keypad.update();
    for event in key_events {
      match event {
        KeyEvent::Pressed { row, col, key, .. } => {
          info!("Key PRESSED:  '{}' [Row {}, Col {}]", key, row, col);
        }
        KeyEvent::Released { row, col, key, .. } => {
          info!("Key RELEASED: '{}' [Row {}, Col {}]", key, row, col);
        }
      }
    }

    // Periodic joystick logging (every 100ms / 5 ticks at 20ms)
    tick_counter = tick_counter.wrapping_add(1);
    if tick_counter % 5 == 0 {
      let dual = joystick.read_dual();
      match dual.active {
        ActiveJoystick::Joystick1 => {
          info!(
            "[J1 *ACTIVE*] X:{:>5.2} Y:{:>5.2} Z:{:>5.2} | [J2  idle  ] X: 0.00 Y: 0.00 Z: 0.00",
            dual.joy1.x, dual.joy1.y, dual.joy1.z,
          );
        }
        ActiveJoystick::Joystick2 => {
          info!(
            "[J1  idle  ] X: 0.00 Y: 0.00 Z: 0.00 | [J2 *ACTIVE*] X:{:>5.2} Y:{:>5.2} Z:{:>5.2}",
            dual.joy2.x, dual.joy2.y, dual.joy2.z,
          );
        }
      }
    }

    Timer::after(Duration::from_millis(20)).await;
  }
}

/// Initializes serial logger, CPU clock and 64KB heap allocator.
fn init() -> Peripherals {
  esp_println::logger::init_logger_from_env();

  let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
  let peripherals = esp_hal::init(config);

  // Initialize heap allocator (reclaiming unused RAM)
  esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);

  peripherals
}
