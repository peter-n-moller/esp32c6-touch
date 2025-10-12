#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{LSGlobalClkSource, LowSpeed};
use esp_hal::time::{Duration, Instant};
use esp_println::println;

use esp_hal::{
    delay::Delay,
    gpio::{Io, Level, Output, OutputConfig},
    ledc::Ledc,
    main,
    rtc_cntl::Rtc,
    spi::{
        master::{Config, Spi},
        Mode,
    },
    time::Rate,
    timer::timg::TimerGroup,
    tsens,
};

// Display driver imports
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle},
};

// Provides the parallel port and display interface builders
use mipidsi::interface::SpiInterface;

// Provides the Display builder
use mipidsi::{models::ST7789, options::ColorInversion, Builder};

use embedded_hal_bus::spi::ExclusiveDevice;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    let mut delay = Delay::new(); // Define the display interface with no chip select
    loop {
        println!("panic!");
        delay.delay(Duration::from_secs(1));
    }
}

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    // generator version: 0.5.0

    let _config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    esp_println::logger::init_logger_from_env();

    println!("start!");

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let peripherals = esp_hal::init(esp_hal::Config::default());

    let mut delay = Delay::new(); // Define the display interface with no chip select

    println!("Disable the RTC and TIMG watchdog timers");
    // Disable the RTC and TIMG watchdog timers
    let mut rtc = Rtc::new(peripherals.LPWR);
    let timer_group0 = TimerGroup::new(peripherals.TIMG0);
    let mut wdt0 = timer_group0.wdt;
    let timer_group1 = TimerGroup::new(peripherals.TIMG1);
    let mut wdt1 = timer_group1.wdt;
    rtc.swd.disable();
    rtc.rwdt.disable();
    wdt0.disable();
    wdt1.disable();

    // Define the reset pin as digital outputs and make it high
    let cs = peripherals.GPIO14;
    let mut cs_output = Output::new(cs, Level::High, OutputConfig::default());

    let mut rst = Output::new(peripherals.GPIO22, Level::Low, OutputConfig::default());
    // rst.set_high();
    // Reset display
    cs_output.set_low();
    delay.delay_millis(50);
    rst.set_low();
    delay.delay_millis(50);
    rst.set_high();
    delay.delay_millis(50);

    // backlight LED Config
    let mut bk_light = Output::new(peripherals.GPIO23, Level::Low, OutputConfig::default());
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    let mut lstimer0 = ledc.timer::<LowSpeed>(esp_hal::ledc::timer::Number::Timer0);
    lstimer0
        .configure(esp_hal::ledc::timer::config::Config {
            duty: esp_hal::ledc::timer::config::Duty::Duty5Bit,
            clock_source: esp_hal::ledc::timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(1),
        })
        .unwrap();

    let mut channel0 = ledc.channel(esp_hal::ledc::channel::Number::Channel0, bk_light);
    channel0
        .configure(esp_hal::ledc::channel::config::Config {
            timer: &lstimer0,
            duty_pct: 10,
            pin_config: esp_hal::ledc::channel::config::PinConfig::PushPull,
        })
        .unwrap();

    channel0.set_duty(80).unwrap();

    // Setup SPI interface
    println!("Setup SPI interface");
    let miso = peripherals.GPIO3;
    let mosi = peripherals.GPIO2;
    let sclk = peripherals.GPIO1;
    let dc = Output::new(peripherals.GPIO15, Level::Low, OutputConfig::default());

    let mut spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(Rate::from_mhz(80))
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_miso(miso) // order matters
    .with_mosi(mosi); // order matters

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs_output).unwrap();

    let mut buffer = [0_u8; 512];
    let di = SpiInterface::new(spi_device, dc, &mut buffer);

    // Define the display from the display interface and initialize it
    let mut display = Builder::new(ST7789, di)
        .reset_pin(rst)
        .display_offset(34, 0)
        .display_size(172, 320)
        .invert_colors(ColorInversion::Normal)
        .init(&mut delay)
        .unwrap();

    // Make the display all black
    display.clear(Rgb565::BLACK).unwrap();

    // Draw a smiley face with white eyes and a red mouth
    draw_smiley(&mut display).unwrap();

    let temperature_sensor =
        tsens::TemperatureSensor::new(peripherals.TSENS, tsens::Config::default()).unwrap();
    let delay = Delay::new();

    loop {
        println!("loop");
        delay.delay(Duration::from_secs(1));
        let temp = temperature_sensor.get_temperature();
        println!("Temperature: {:.2}°C", temp.to_celsius());
        // Do nothing
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}

fn draw_smiley<T: DrawTarget<Color = Rgb565>>(display: &mut T) -> Result<(), T::Error> {
    // Draw the left eye as a circle located at (50, 100), with a diameter of 40, filled with white
    println!("draw_smiley");
    Circle::new(Point::new(50, 100), 40)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw the right eye as a circle located at (50, 200), with a diameter of 40, filled with white
    Circle::new(Point::new(50, 200), 40)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw an upside down red triangle to represent a smiling mouth
    Triangle::new(
        Point::new(130, 140),
        Point::new(130, 200),
        Point::new(160, 170),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
    .draw(display)?;

    // Cover the top part of the mouth with a black triangle so it looks closed instead of open
    Triangle::new(
        Point::new(130, 150),
        Point::new(130, 190),
        Point::new(150, 170),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
    .draw(display)?;

    println!("done draw smiley!");

    Ok(())
}
