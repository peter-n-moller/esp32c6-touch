# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an ESP32-C6 embedded project for the Waveshare ESP32-C6-Touch-LCD-1.47 hardware. It demonstrates:
- SPI LCD display driver (mipidsi with ILI9341Rgb565 model)
- Custom I2C touch controller driver (AXS5106L)
- PWM backlight control via LEDC peripheral
- Temperature sensor and ADC battery voltage monitoring

Target: `riscv32imac-unknown-none-elf` (RISC-V 32-bit, no operating system)

## Build and Flash Commands

### Build
```bash
cargo build                  # Debug build with opt-level=s
cargo build --release        # Release build with LTO and size optimization
```

### Flash and Monitor
```bash
cargo run                    # Build, flash, and monitor (uses espflash configured in .cargo/config.toml)
cargo run --release          # Flash release build
```

The runner is configured to use `espflash flash --monitor --chip esp32c6`, so `cargo run` automatically flashes and opens the serial monitor.

### Check Code
```bash
cargo check                  # Fast syntax/type checking without building
cargo clippy                 # Run linter (note: clippy::mem_forget is denied)
```

## Architecture

### Hardware Abstraction
- **esp-hal**: Main hardware abstraction layer for ESP32-C6
- **embedded-hal**: Trait-based abstractions for portable drivers (I2C, SPI)
- **no_std**: No standard library - uses `alloc` for heap allocations (64KB heap configured)

### Key Components

#### Display System (main.rs:93-178)
1. **Reset sequence**: CS low → RST toggle → delay
2. **SPI setup**: 80MHz, Mode 0, using GPIO1/2/3/14/15
3. **Display driver**: `mipidsi::Builder` with ILI9341Rgb565
   - Display offset: (34, 0)
   - Size: 172x320 pixels
   - Horizontal flip orientation
   - BGR color order
4. **Backlight PWM**: LEDC Timer0, Channel0, 24kHz, configurable duty cycle (constant: `BACKLIGHT_DUTY`)

#### Touch System (main.rs:181-223)
- **Driver**: `axs5106l.rs` - Custom I2C driver for AXS5106L touch controller
- **I2C**: 400kHz on GPIO18 (SDA) / GPIO19 (SCL)
- **Interrupt**: GPIO21 input with pull-up (polled, not hardware interrupt)
- **Reset**: GPIO20 output (200ms low, 200ms high before init)
- **Coordinate transformation**: Supports 4 rotation modes (0°, 90°, 180°, 270°)

#### Sensor Monitoring (main.rs:236-243)
- **ADC**: GPIO0 for battery voltage (11dB attenuation, conversion: `VAL_TO_VOLT = 5.0 / 4096.0`)
- **Temperature**: Built-in TSENS peripheral

#### Main Loop (main.rs:248-291)
- 50ms polling interval
- Touch interrupt polling (active LOW)
- Sensor readings displayed on screen using `embedded-graphics`

### Module Structure
- `src/bin/main.rs`: Application entry point with hardware initialization and main loop
- `src/lib.rs`: Library crate root (currently only exports `axs5106l` module)
- `src/axs5106l.rs`: AXS5106L touch controller driver
  - I2C register-based communication
  - Multi-touch support (up to 5 points)
  - Interrupt flag management (manual, not hardware-driven)
  - Coordinate transformation based on display rotation
  - See TOUCH_DRIVER_USAGE.md for detailed API documentation

### Build Configuration

#### Cargo Profile Settings
- **Debug**: `opt-level = "s"` (size optimization - debug is too slow for embedded)
- **Release**: LTO enabled, single codegen unit, size optimization, frame pointers enabled

#### Target Configuration (.cargo/config.toml)
- Default target: `riscv32imac-unknown-none-elf`
- Build-std: `["alloc", "core"]` (rebuild stdlib for target)
- Stack protector enabled (`-Z stack-protector=all`)
- Frame pointers forced for backtrace support

#### Watchdog Timers
All watchdog timers (RTC SWD, RTC RWDT, TIMG0, TIMG1) are explicitly disabled in the initialization sequence (main.rs:82-90).

### Display Driver Workaround
The hardware uses a JD9853 LCD controller, but this driver is not available in `mipidsi`. The project currently uses `ILI9341Rgb565` as a compatible alternative with manual offset configuration.

## Important Constraints

### Safety
- `mem::forget` is denied via clippy lint - esp_hal types holding DMA buffers must not be forgotten
- No heap operations in the middle of DMA transfers

### Hardware-Specific
- Display offset must be set to (34, 0) for correct rendering
- Touch controller I2C address: 0x63
- Backlight duty cycle is configurable via `BACKLIGHT_DUTY` constant

### Embedded Development
- All code must be `no_std` compatible
- Heap allocations limited to 64KB
- No floating point in tight loops (sensor readings use f32 but only at 50ms intervals)
