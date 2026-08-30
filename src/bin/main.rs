#![no_std]
#![no_main]
#![deny(
  clippy::mem_forget,
  reason = "mem::forget is generally not safe to do with esp_hal types holding buffers."
)]
#![deny(clippy::large_stack_frames)]

extern crate alloc;

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::peripherals::Peripherals;
use esp_hal::timer::timg::{MwdtStage, TimerGroup};
use panic_rtt_target as _;

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

  info!("ESP32-C6 Embassy Runtime initialized!");

  // Main Watchdog Timer (MWDT) setup
  wdt0.enable();
  wdt0.set_timeout(MwdtStage::Stage0, esp_hal::time::Duration::from_secs(30));

  // Initialize LED on GPIO4
  let mut led = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
  let mut led_state = false;

  loop {
    // Feed watchdog timer regularly
    wdt0.feed();

    led_state = !led_state;
    led.set_level(if led_state { Level::High } else { Level::Low });
    info!("Heartbeat tick (LED: {})", led_state);

    Timer::after(Duration::from_secs(1)).await;
  }
}

/// Initializes RTT logger, CPU clock, strap pin guards and 64KB heap allocator.
fn init() -> Peripherals {
  rtt_target::rtt_init_defmt!();

  let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
  let peripherals = esp_hal::init(config);

  // Strapping pins note: GPIO4, GPIO5, GPIO8, GPIO9, GPIO15 are chip bootstrap pins.
  // The following pins are reserved internally or unused on standard ESP32-C6-MINI modules:
  let _ = peripherals.GPIO24;
  let _ = peripherals.GPIO25;
  let _ = peripherals.GPIO26;
  let _ = peripherals.GPIO27;
  let _ = peripherals.GPIO28;
  let _ = peripherals.GPIO29;
  let _ = peripherals.GPIO30;

  // Initialize heap allocator (reclaiming unused RAM)
  esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);

  peripherals
}
