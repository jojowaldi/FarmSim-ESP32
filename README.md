# Universal Rust ESP32-C6 Project Template (Embassy Async + no_std)

A production-ready, universal project template for building asynchronous firmware in **Rust** (`no_std`) for the **Espressif ESP32-C6** (RISC-V 32-bit core), powered by the **Embassy async runtime**, **ESP-HAL / ESP-RTOS**, and **defmt RTT logging**.

---

## 📋 Features & Architecture

- ⚡ **Asynchronous Runtime**: Full `embassy-executor` + `esp-rtos` scheduler integration.
- 🪵 **Defmt + RTT Logging**: Ultra-fast, tokenized structured logging via JTAG / USB-Serial-JTAG (`panic-rtt-target`, `rtt-target`).
- 🛡️ **Reliability & Watchdog**: Main Watchdog Timer (MWDT) setup & periodic feeding patterns.
- 🛠️ **Memory Utilities**: Safe static cell allocation macros (`mk_static!`) for Embassy task memory reuse.
- ⚙️ **Barebones & Modular**: Minimal baseline for quick adaptation to any peripheral or networking stack.

---

## 🗂️ Project Structure

```text
template/
├── .cargo/
│   └── config.toml          # Target riscv32imac-unknown-none-elf, probe-rs runner, defmt env
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
├── build.rs                 # Linker scripts (-Tdefmt.x, -Tlinkall.x) & diagnostic helper
├── Cargo.toml               # Dependencies & MCU build profiles
├── rust-toolchain.toml      # Nightly toolchain & riscv32 target specification
└── rustfmt.toml             # Formatting preferences
```

---

## 🚀 Getting Started

### 1. Prerequisites

1. Install **Rust** via [rustup.rs](https://rustup.rs/):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. The `rust-toolchain.toml` file will automatically select `nightly` and install the `riscv32imac-unknown-none-elf` target with `rust-src`. If needed manually:
   ```bash
   rustup toolchain install nightly
   rustup target add riscv32imac-unknown-none-elf --toolchain nightly
   rustup component add rust-src --toolchain nightly
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

### 2. Bootstrapping a New Project

1. Update the package name in `Cargo.toml` as needed:
   ```toml
   [package]
   name = "my-esp32c6-project"
   version = "0.1.0"
   edition = "2024"

   [[bin]]
   name = "my-esp32c6-project"
   path = "./src/bin/main.rs"
   ```
2. Build the project:
   ```bash
   cargo build
   ```

---

### 3. Flashing & Running

Connect your ESP32-C6 board via USB (native USB-Serial-JTAG port).

#### Using `cargo run` (probe-rs)
The `.cargo/config.toml` is configured to run `probe-rs run`:
```bash
cargo run
```
You will immediately see real-time `defmt` log output in your terminal!

#### Using `espflash`
```bash
espflash flash --chip esp32c6 --monitor target/riscv32imac-unknown-none-elf/debug/esp-c6-template
```

---

## 📌 Hardware Pinout & Architecture Notes

### ESP32-C6 Strapping Pins
Be mindful when assigning the following pins as they affect chip boot modes:
- **GPIO4**, **GPIO5**, **GPIO8**, **GPIO9**, **GPIO15**
- **GPIO8 & GPIO9**: Control boot mode (SPI Boot vs Download Boot). Keep high or floating with default pull-ups during power-on.

---

## 💡 Key Architectural Patterns

### 1. Static Allocation for Embassy Tasks (`mk_static!`)
Embassy tasks require `'static` lifetimes for references. Use the provided macros in `src/utils.rs`:
```rust
use esp_c6_template::mk_static;

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
| `clippy::large_stack_frames` | Large buffer allocated on the stack | Move large buffers to `static` using `mk_static!` or annotate with `#[allow(clippy::large_stack_frames)]` if in main. |
| `probe-rs: Device not found` | USB permissions or incorrect driver | On Windows, ensure WinUSB is installed for the USB JTAG/Serial device via [Zadig](https://zadig.akeo.ie/). On Linux, add udev rules for Espressif VID:PID (`303a:1001`). |

---

## 📄 License
Licensed under either Apache License, Version 2.0 or MIT License at your option.

