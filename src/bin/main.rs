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
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::peripherals::Peripherals;
use esp_hal::timer::timg::{MwdtStage, TimerGroup};
use log::info;

use esp32s3_template::joystick::Joystick;

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

  // Initialize on-board LED (GPIO8 on ESP32-S3 Super-Mini)
  let mut led = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());
  let mut led_state = false;

  // Initialize 3-axis Joystick on ADC1 (X=GPIO1, Y=GPIO2, Z=GPIO4)
  let mut joystick = Joystick::new(
    peripherals.ADC1,
    peripherals.GPIO1,
    peripherals.GPIO2,
    peripherals.GPIO4,
  );

  // Calibrate center position in rest mode (64 samples)
  info!("Calibrating joystick center (keep joystick at rest)...");
  joystick.calibrate_center(64);
  info!(
    "Calibration complete! Centers: X={} mV, Y={} mV, Z={} mV",
    joystick.config.x.center_mv,
    joystick.config.y.center_mv,
    joystick.config.z.center_mv,
  );

  let mut tick_counter: u32 = 0;

  loop {
    // Feed watchdog timer regularly
    wdt0.feed();

    // Read joystick values (raw millivolts and normalized -1.0..1.0)
    let reading = joystick.read();

    // Toggle heartbeat LED every ~1 second (every 10 ticks at 100ms)
    tick_counter = tick_counter.wrapping_add(1);
    if tick_counter % 10 == 0 {
      led_state = !led_state;
      led.set_level(if led_state { Level::High } else { Level::Low });
    }

    // Output joystick values to serial console in human-readable format
    info!(
      "Joystick | X: {:>5.2} (raw: {:>4} mV) | Y: {:>5.2} (raw: {:>4} mV) | Z: {:>5.2} (raw: {:>4} mV)",
      reading.x, reading.raw_x_mv,
      reading.y, reading.raw_y_mv,
      reading.z, reading.raw_z_mv,
    );

    Timer::after(Duration::from_millis(100)).await;
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
