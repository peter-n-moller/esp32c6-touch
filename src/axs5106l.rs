#![allow(dead_code)]

use embedded_hal::i2c::I2c;

// Import standard library traits for derive
use Option::{None, Some};
use Result::Ok;
use core::clone::Clone;
use core::cmp::min;
use core::default::Default;
use core::marker::Copy;
use core::option::Option;
use core::prelude::rust_2024::derive;
use core::result::Result;
use esp_println::println;

/// Maximum number of touch points supported
const MAX_TOUCH_POINTS: usize = 5;

/// I2C address of the AXS5106L touch controller
const AXS5106L_ADDR: u8 = 0x63;

/// Register address for device ID
const AXS5106L_ID_REG: u8 = 0x08;

/// Register address for touch data
const AXS5106L_TOUCH_DATA_REG: u8 = 0x01;

/// Touch point coordinates
#[derive(Clone, Copy, Default)]
pub struct Coordinates {
    pub x: u16,
    pub y: u16,
}

/// Touch data containing all touch points
#[derive(Clone, Default)]
pub struct TouchData {
    pub coords: [Coordinates; MAX_TOUCH_POINTS],
    pub touch_num: u8,
}

/// Display rotation modes
#[derive(Clone, Copy)]
pub enum Rotation {
    Rotate0 = 0,
    Rotate90 = 1,
    Rotate180 = 2,
    Rotate270 = 3,
}

/// AXS5106L touch controller driver
pub struct Axs5106l<I2C> {
    i2c: I2C,
    width: u16,
    height: u16,
    rotation: Rotation,
    touch_data: TouchData,
    touch_int_flag: bool,
}

impl<I2C, E> Axs5106l<I2C>
where
    I2C: I2c<Error = E>,
{
    /// Create a new AXS5106L driver instance
    ///
    /// # Arguments
    /// * `i2c` - I2C bus instance
    /// * `rotation` - Display rotation
    /// * `width` - Display width in pixels
    /// * `height` - Display height in pixels
    pub fn new(i2c: I2C, rotation: Rotation, width: u16, height: u16) -> Self {
        Self {
            i2c,
            width,
            height,
            rotation,
            touch_data: TouchData::default(),
            touch_int_flag: false,
        }
    }

    /// Initialize the touch controller
    ///
    /// Reads the device ID register to verify communication
    pub fn init(&mut self) -> Result<(), E> {
        let mut data = [0u8; 3];
        self.i2c_read(AXS5106L_ID_REG, &mut data)?;

        // If data[0] is not zero, the device responded
        if data[0] != 0 {
            // Device ID read successfully
        }

        Ok(())
    }

    /// Read from an I2C register
    fn i2c_read(&mut self, reg_addr: u8, data: &mut [u8]) -> Result<(), E> {
        self.i2c.write_read(AXS5106L_ADDR, &[reg_addr], data)
    }

    /// Write to an I2C register
    #[allow(dead_code)]
    fn i2c_write(&mut self, reg_addr: u8, data: &[u8]) -> Result<(), E> {
        let mut buffer = [0u8; 33]; // Max length: 1 (reg) + 32 (data)
        buffer[0] = reg_addr;
        buffer[1..1 + data.len()].copy_from_slice(data);
        self.i2c.write(AXS5106L_ADDR, &buffer[..1 + data.len()])
    }

    /// Set the interrupt flag (to be called from interrupt handler)
    pub fn set_interrupt(&mut self) {
        self.touch_int_flag = true;
    }

    /// Clear the interrupt flag
    pub fn clear_interrupt(&mut self) {
        self.touch_int_flag = false;
    }

    /// Check if there's a pending touch interrupt
    pub fn has_interrupt(&self) -> bool {
        self.touch_int_flag
    }

    /// Read touch data from the controller
    ///
    /// This should be called after an interrupt occurs
    pub fn read_touch(&mut self) -> Result<(), E> {
        if !self.touch_int_flag {
            return Ok(());
        }

        self.touch_int_flag = false;

        let mut data = [0u8; 14];
        self.i2c_read(AXS5106L_TOUCH_DATA_REG, &mut data)?;

        self.touch_data.touch_num = data[1];

        if self.touch_data.touch_num == 0 {
            return Ok(());
        }

        // Parse touch coordinates
        for i in 0..min(self.touch_data.touch_num, MAX_TOUCH_POINTS as u8) as usize {
            let base = 2 + i * 6;

            // Extract 12-bit X coordinate
            self.touch_data.coords[i].x = ((data[base] as u16 & 0x0F) << 8) | data[base + 1] as u16;

            // Extract 12-bit Y coordinate
            self.touch_data.coords[i].y =
                ((data[base + 2] as u16 & 0x0F) << 8) | data[base + 3] as u16;
        }

        Ok(())
    }

    /// Get touch coordinates with rotation applied
    ///
    /// Returns None if there are no touches or if the internal touch data is invalid
    pub fn get_coordinates(&self) -> Option<TouchData> {
        if self.touch_data.touch_num == 0 {
            return None;
        }

        let mut transformed = self.touch_data.clone();

        // Apply rotation transformation to each touch point
        for i in 0..min(self.touch_data.touch_num, MAX_TOUCH_POINTS as u8) as usize {
            let (x, y) = match self.rotation {
                Rotation::Rotate0 => {
                    // Default orientation
                    (
                        self.width
                            .saturating_sub(1)
                            .saturating_sub(self.touch_data.coords[i].x),
                        self.touch_data.coords[i].y,
                    )
                }
                Rotation::Rotate90 => {
                    // 90 degrees clockwise
                    (self.touch_data.coords[i].y, self.touch_data.coords[i].x)
                }
                Rotation::Rotate180 => {
                    // 180 degrees
                    (
                        self.touch_data.coords[i].x,
                        self.height
                            .saturating_sub(1)
                            .saturating_sub(self.touch_data.coords[i].y),
                    )
                }
                Rotation::Rotate270 => {
                    // 270 degrees clockwise
                    (
                        self.height
                            .saturating_sub(1)
                            .saturating_sub(self.touch_data.coords[i].x),
                        self.width
                            .saturating_sub(1)
                            .saturating_sub(self.touch_data.coords[i].y),
                    )
                }
            };

            transformed.coords[i].x = x;
            transformed.coords[i].y = y;
        }

        Some(transformed)
    }

    /// Get the number of current touches
    pub fn touch_count(&self) -> u8 {
        self.touch_data.touch_num
    }

    /// Check if any touches are currently detected
    pub fn has_touches(&self) -> bool {
        self.touch_data.touch_num > 0
    }
}
