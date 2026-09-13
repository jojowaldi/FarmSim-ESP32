#![no_std]
#![no_main]
#![deny(
  clippy::mem_forget,
  reason = "mem::forget is generally not safe to do with esp_hal types holding buffers."
)]
#![allow(clippy::large_stack_frames)]

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
  joystick::{ButtonEdge, DebouncedButton, Joystick},
  keyboard::{KeyboardReport, KEYBOARD_REPORT_DESCRIPTOR},
  matrix::{KeyEvent, MatrixKeypad4x4},
  mouse::{MouseReport, MOUSE_REPORT_DESCRIPTOR},
  profile::ControllerProfile,
  mk_static,
};

// App descriptor required by esp-idf bootloader
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
  clippy::large_stack_frames,
  reason = "UsbDevice structure holds endpoint descriptors for composite HID interfaces"
)]
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

  // Static buffers for USB OTG Composite Device (Joystick + Keyboard + Mouse)
  let ep_out_buffer = mk_static!([u8; 256], [0u8; 256]);
  let config_descriptor = mk_static!([u8; 256], [0u8; 256]);
  let bos_descriptor = mk_static!([u8; 256], [0u8; 256]);
  let msos_descriptor = mk_static!([u8; 256], [0u8; 256]);
  let control_buf = mk_static!([u8; 64], [0u8; 64]);
  let joy_hid_state = mk_static!(HidState, HidState::new());
  let kbd_hid_state = mk_static!(HidState, HidState::new());
  let mouse_hid_state = mk_static!(HidState, HidState::new());

  // Initialize USB OTG peripheral on native USB pins (DP=GPIO20, DM=GPIO19)
  let usb_peri = Usb::new(peripherals.USB0, peripherals.GPIO20, peripherals.GPIO19);
  let usb_driver = Driver::new(usb_peri, ep_out_buffer, OtgConfig::default());

  // USB Device configuration (Composite Controller: Joystick + Keyboard + Mouse)
  let mut usb_config = UsbConfig::new(0x1209, 0x2001);
  usb_config.manufacturer = Some("FarmStick");
  usb_config.product = Some("FarmStick Controller");
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

  // Interface 1: USB HID Joystick
  let joy_config = HidConfig {
    report_descriptor: GAMEPAD_REPORT_DESCRIPTOR,
    request_handler: None,
    poll_ms: 5,
    max_packet_size: 64,
  };
  let mut joy_writer = HidWriter::<_, 8>::new(&mut builder, joy_hid_state, joy_config);

  // Interface 2: USB HID Keyboard (Macropad F13..F24)
  let kbd_config = HidConfig {
    report_descriptor: KEYBOARD_REPORT_DESCRIPTOR,
    request_handler: None,
    poll_ms: 5,
    max_packet_size: 64,
  };
  let mut kbd_writer = HidWriter::<_, 8>::new(&mut builder, kbd_hid_state, kbd_config);

  // Interface 3: USB HID Mouse (3D Look-Around)
  let mouse_config = HidConfig {
    report_descriptor: MOUSE_REPORT_DESCRIPTOR,
    request_handler: None,
    poll_ms: 5,
    max_packet_size: 64,
  };
  let mut mouse_writer = HidWriter::<_, 4>::new(&mut builder, mouse_hid_state, mouse_config);

  let usb_device = builder.build();

  // Spawn background USB device task
  spawner.spawn(usb_task(usb_device).unwrap());

  // Wait until USB endpoints are enabled by host
  joy_writer.ready().await;
  kbd_writer.ready().await;
  mouse_writer.ready().await;

  let mut current_profile = ControllerProfile::FarmSim;
  let mut shortcut_triggered = false;
  let mut last_kbd_report = KeyboardReport::default();
  let mut last_mouse_had_activity = false;
  let mut wheel_accum = 0.0f32;

  loop {
    // Feed watchdog timer regularly
    wdt0.feed();

    // 1. Scan push button on GPIO 13 (Joystick Switch Button)
    let switch_edge = switch_button.update();
    let switch_held = switch_button.is_held();

    // 2. Scan 4x4 matrix keypad
    let key_events = keypad.update();
    let buttons_bitmask = keypad.debounced_state();

    // 3. Check for Profile Switching Shortcut: Joystick Switch (GPIO 13) + Button 4
    if switch_held {
      for ev in &key_events {
        if let KeyEvent::Pressed { index, key, .. } = ev {
          // Button 4 can be 4th button in row (index 3) or key labeled '4' (index 4)
          if *index == 3 || *index == 4 || *key == '4' {
            current_profile.toggle();
            shortcut_triggered = true;
            log::info!("Switched active profile to: {}", current_profile.label());

            // Clear states on the newly inactive profile
            match current_profile {
              ControllerProfile::FarmSim => {
                let _ = kbd_writer.write(&KeyboardReport::default().to_bytes()).await;
                let _ = mouse_writer.write(&MouseReport::default().to_bytes()).await;
                last_kbd_report = KeyboardReport::default();
                last_mouse_had_activity = false;
              }
              ControllerProfile::Macropad => {
                let _ = joy_writer.write(&GamepadReport::default().to_bytes()).await;
              }
            }
            break;
          }
        }
      }
    }

    // Handle release of Switch Button (single tap vs shortcut release)
    if switch_edge == ButtonEdge::Released {
      if shortcut_triggered {
        // Shortcut was used; do not trigger single-click action
        shortcut_triggered = false;
      } else if current_profile == ControllerProfile::FarmSim {
        // Normal single tap in FarmSim mode toggles J1 <-> J2
        joystick.toggle_mode();
        log::info!("FarmSim mode toggled to: {}", joystick.active_mode().label());
      }
    }

    // 4. Read physical joystick
    let joy_reading = joystick.read();

    // Suppress shortcut buttons while switch button is held so they don't fire unwanted actions
    let active_buttons = if switch_held {
      buttons_bitmask & !((1 << 3) | (1 << 4))
    } else {
      buttons_bitmask
    };

    // 5. Dispatch based on current profile
    match current_profile {
      ControllerProfile::FarmSim => {
        let report = GamepadReport::new(joystick.active_mode(), joy_reading, active_buttons);
        let _ = joy_writer.write(&report.to_bytes()).await;
      }
      ControllerProfile::Macropad => {
        // A) Keyboard (F13..F24 Macropad)
        let kbd_report = KeyboardReport::from_buttons_bitmask(active_buttons);
        if kbd_report != last_kbd_report {
          let _ = kbd_writer.write(&kbd_report.to_bytes()).await;
          last_kbd_report = kbd_report;
        }

        // B) Mouse (3D Look-Around & Orbit)
        // If switch button is held without shortcut, it acts as Middle Mouse Button (MMB orbit in CAD/Blender)
        let middle_click = switch_held && !shortcut_triggered;
        let mouse_report =
          MouseReport::from_joystick(joy_reading, middle_click, &mut wheel_accum);

        if mouse_report.has_activity() {
          let _ = mouse_writer.write(&mouse_report.to_bytes()).await;
          last_mouse_had_activity = true;
        } else if last_mouse_had_activity {
          // Send one release report to reset host deltas
          let _ = mouse_writer.write(&MouseReport::default().to_bytes()).await;
          last_mouse_had_activity = false;
        }
      }
    }

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

