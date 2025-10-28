# AXS5106L Touch Driver Usage

The C++ driver has been successfully converted to Rust. The new driver is located in `src/axs5106l.rs`.

## Features

- **I2C Communication**: Uses `embedded-hal` I2C traits for hardware abstraction
- **Multi-touch Support**: Handles up to 5 simultaneous touch points
- **Rotation Support**: Transforms coordinates for 0°, 90°, 180°, and 270° rotations
- **Interrupt Handling**: Flag-based interrupt management
- **No-std Compatible**: Works in embedded `no_std` environments

## Basic Usage Example

```rust
use display_test::axs5106l::{Axs5106l, Rotation};
use esp_hal::i2c::master::I2c;
use esp_hal::gpio::{Input, Pull};

// Initialize I2C bus
let i2c = I2c::new(
    peripherals.I2C0,
    esp_hal::i2c::master::Config::default()
        .with_frequency(Rate::from_khz(400)),
)
.with_sda(sda_pin)
.with_scl(scl_pin);

// Create touch driver instance
let mut touch = Axs5106l::new(
    i2c,
    Rotation::Rotate0,  // Set display rotation
    320,                 // Display width
    172                  // Display height
);

// Initialize the touch controller
touch.init().expect("Failed to initialize touch controller");

// Set up interrupt pin
let touch_int = Input::new(peripherals.GPIO_X, Pull::Up);

// In your main loop or interrupt handler:
loop {
    // Check if touch interrupt occurred
    if touch_int.is_low() {
        touch.set_interrupt();
    }

    // Read touch data if interrupt flag is set
    if touch.has_interrupt() {
        touch.read_touch().expect("Failed to read touch data");
        
        // Get transformed coordinates
        if let Some(touch_data) = touch.get_coordinates() {
            for i in 0..touch_data.touch_num {
                let coord = touch_data.coords[i as usize];
                println!("Touch {}: x={}, y={}", i, coord.x, coord.y);
            }
        }
    }
    
    delay.delay_millis(10);
}
```

## Key Differences from C++ Driver

1. **Type Safety**: Uses Rust's type system for better compile-time guarantees
2. **Error Handling**: Returns `Result` types instead of boolean success flags
3. **Ownership**: No global variables - state is managed through the `Axs5106l` struct
4. **Interrupt Management**: Manual flag setting (call `set_interrupt()` from your interrupt handler)
5. **Hardware Abstraction**: Uses `embedded-hal` traits for I2C, making it portable across different hardware

## API Reference

### `Axs5106l::new(i2c, rotation, width, height)`
Creates a new touch driver instance.

### `init(&mut self) -> Result<(), E>`
Initializes the touch controller by reading the device ID.

### `set_interrupt(&mut self)`
Sets the interrupt flag (call this from your interrupt handler).

### `has_interrupt(&self) -> bool`
Checks if there's a pending touch interrupt.

### `read_touch(&mut self) -> Result<(), E>`
Reads touch data from the controller (clears interrupt flag automatically).

### `get_coordinates(&self) -> Option<TouchData>`
Returns transformed touch coordinates based on display rotation, or `None` if no touches.

### `touch_count(&self) -> u8`
Returns the number of current touches.

### `has_touches(&self) -> bool`
Checks if any touches are currently detected.

## Notes

- The driver requires an I2C bus that implements the `embedded_hal::i2c::I2c` trait
- Interrupt handling is done via flags - you need to call `set_interrupt()` from your interrupt handler
- For hardware reset, manage the reset pin externally before calling `init()`
- The driver is `no_std` compatible and suitable for embedded systems
