# ESP32-S3 USB HID Gamepad Controller (Embassy Async + no_std)

A high-performance, asynchronous **USB HID Gamepad Controller** firmware in **Rust** (`no_std`) for the **Espressif ESP32-S3 / ESP32-S3 Super-Mini**, powered by **Embassy async runtime**, **ESP-HAL / ESP-RTOS**, and **embassy-usb**.

---

## 📋 Features & Architecture

- 🎮 **Native USB HID Gamepad**: Plug-and-play USB Gamepad recognized natively on Windows, Linux, and macOS without drivers.
- 🕹️ **Dual-Mode 3-Axis Joystick**: X, Y, and Z axes with toggleable virtual joystick banks (**J1**: X/Y/Z vs **J2**: Rx/Ry/Rz) via Pin 13 button.
- 🖲️ **16 Generic Gamepad Buttons**: 4x4 Matrix Keypad with anti-ghosting diode support and debouncing, mapped to standard Buttons 1..16.
- ⚡ **200 Hz High-Speed Polling**: Ultra-low latency 5ms report updates over USB HID.
- ⚙️ **Automatic Boot Calibration**: Factory eFuse ADC curve calibration + automatic rest-position zero-point calibration at startup.

---

## 🗂️ Project Structure

```text
template/
├── .cargo/
│   └── config.toml          # Target xtensa-esp32s3-none-elf, probe-rs runner, defmt env
├── .vscode/
│   ├── launch.json          # Probe-rs debug configurations with RTT defmt channel
│   ├── settings.json        # Rust-analyzer configuration
│   └── tasks.json           # Build & run tasks
├── .zed/
│   └── settings.json        # Zed editor LSP settings
├── src/
│   ├── bin/
│   │   └── main.rs          # Application entry point, RTOS init & main task loop
│   ├── lib.rs               # Library root
│   └── utils.rs             # Static allocation macros (mk_static!)
├── .clippy.toml             # Stack size limit check (1024 bytes)
├── .gitignore               # Standard Rust ignores
├── build.rs                 # Linker scripts (-Tdefmt.x, -Tlinkall.x)
├── Cargo.toml               # Dependencies & MCU build profiles
├── rust-toolchain.toml      # Espressif Xtensa toolchain channel (esp)
└── rustfmt.toml             # Formatting preferences
```

---

## 🚀 Getting Started

### 1. Prerequisites

1. Install **Rust** via [rustup.rs](https://rustup.rs/):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. Install the **Espressif Xtensa toolchain** using `espup`:
   ```bash
   cargo install espup --locked
   espup install --targets esp32s3
   ```
3. Install **probe-rs** (recommended for flashing and RTT defmt debugging):
   ```bash
   cargo install probe-rs-tools --locked
   ```
4. (Optional) Install **espflash**:
   ```bash
   cargo install espflash --locked
   ```

---

### 2. Building & Flashing

Connect your ESP32-S3 Super-Mini board via USB (native USB-Serial-JTAG port).

#### Using `cargo build`
```bash
cargo build
```

#### Using `cargo run` (probe-rs)
The `.cargo/config.toml` is configured to run `probe-rs run`:
```bash
cargo run
```
You will immediately see real-time `defmt` log output in your terminal!

#### Using `espflash`
```bash
espflash flash --chip esp32s3 --monitor target/xtensa-esp32s3-none-elf/debug/esp32s3-template
```

---

## 📌 Hardware Pinout & Architecture Notes

### ESP32-S3 Strapping Pins
Be mindful when assigning the following pins as they affect chip boot modes:
- **GPIO0**: Boot mode (Pull-up = SPI Boot, Button to GND = Download Mode)
- **GPIO45**: VDD_SPI power domain voltage
- **GPIO46**: ROM boot message logging
- **GPIO3**: JTAG strap

### ESP32-S3 Super-Mini Pinout Highlights
- **Joystick**: `X = GPIO 1`, `Y = GPIO 2`, `Z = GPIO 4` (ADC1 Channels 0, 1, 3)
- **Joystick Mode Switch Button**: `GPIO 13` (Active-Low mit internem Pull-Up, schaltet J1 <-> J2)
- **4x4 Keypad Matrix**:
  - **Rows (Outputs)**: `GPIO 5`, `GPIO 6`, `GPIO 7`, `GPIO 8`
  - **Columns (Inputs)**: `GPIO 9`, `GPIO 10`, `GPIO 11`, `GPIO 12`
- **Native USB-JTAG/Serial**: `GPIO 19` (D-) / `GPIO 20` (D+)

---

## 💡 Key Architectural Patterns

### 1. Static Allocation for Embassy Tasks (`mk_static!`)
Embassy tasks require `'static` lifetimes for references. Use the provided macros in `src/utils.rs`:
```rust
use esp32s3_template::mk_static;

let buffer = mk_static!([u8; 1024], [0u8; 1024]);
```

### 2. Watchdog Timer (MWDT)
Always feed the watchdog timer periodically in long-running loops or tasks:
```rust
wdt0.feed();
```

---

## 🐛 Troubleshooting

| Problem | Cause | Solution |
| :--- | :--- | :--- |
| `undefined symbol _defmt_` | Missing linker script or logger import | Ensure `build.rs` contains `-Tdefmt.x` and `panic_rtt_target as _;` is in `main.rs`. |
| `undefined symbol malloc / free` | Heap allocator not initialized | Ensure `esp_alloc::heap_allocator!` is called in `init()`. |
| `linker xtensa-esp32s3-elf-gcc not found` | Xtensa toolchain not in PATH | Run `espup install --targets esp32s3` and ensure the toolchain path is in your environment. |
| `probe-rs: Device not found` | USB permissions or incorrect driver | On Windows, ensure WinUSB is installed for the USB JTAG/Serial device via [Zadig](https://zadig.akeo.ie/). On Linux, add udev rules for Espressif VID:PID (`303a:1001`). |

---

## 📄 License
Licensed under either Apache License, Version 2.0 or MIT License at your option.


