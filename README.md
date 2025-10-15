# Test project for ESP32c6 Touch

This is the used [hardware](https://www.waveshare.com/wiki/ESP32-C6-Touch-LCD-1.47>)

It uses a JD9853 LCD display driver, but that doesn't exists,
in the mipidsi driver for rust, so I'm currently running with
ILI9341Rgb565 driver. However, it seems the red and blue channels are shifted.

The display drivers seems to be able to change the color format from RGB to BGR.
Check out that further.
