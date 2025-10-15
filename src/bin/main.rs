#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#[macro_use]
extern crate alloc;

use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{LSGlobalClkSource, LowSpeed};
use esp_hal::time::Duration;
use esp_println::println;

use esp_hal::{
    analog::adc::{Adc, AdcConfig, Attenuation},
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
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
    mono_font::{ascii::FONT_6X9, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle},
    text::Text,
};

// Provides the parallel port and display interface builders
use mipidsi::interface::SpiInterface;

use mipidsi::options::Orientation;
// Provides the Display builder
use mipidsi::{models::ILI9341Rgb565, options::ColorInversion, Builder};

use embedded_hal_bus::spi::ExclusiveDevice;

// Constants
const VAL_TO_VOLT: f32 = 5.0 / 4096.0;
const BACKLIGHT_DUTY: u8 = 80;
const DISPLAY_WIDTH: u16 = 172;
const DISPLAY_HEIGHT: u16 = 320;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    let delay = Delay::new();
    loop {
        println!("panic!");
        delay.delay(Duration::from_secs(1));
    }
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
#[main]
fn main() -> ! {
    // ========================================
    // SYSTEM INITIALIZATION
    // ========================================
    let _config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    esp_println::logger::init_logger_from_env();
    println!("start!");
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let peripherals = esp_hal::init(esp_hal::Config::default());
    let mut delay = Delay::new();

    // ========================================
    // DISABLE WATCHDOG TIMERS
    // ========================================
    println!("Disable the RTC and TIMG watchdog timers");
    let mut rtc = Rtc::new(peripherals.LPWR);
    let timer_group0 = TimerGroup::new(peripherals.TIMG0);
    let mut wdt0 = timer_group0.wdt;
    let timer_group1 = TimerGroup::new(peripherals.TIMG1);
    let mut wdt1 = timer_group1.wdt;
    rtc.swd.disable();
    rtc.rwdt.disable();
    wdt0.disable();
    wdt1.disable();

    // ========================================
    // DISPLAY HARDWARE SETUP
    // ========================================

    // Initialize display control pins
    let cs = peripherals.GPIO14;
    let mut cs_output = Output::new(cs, Level::High, OutputConfig::default());
    let mut rst = Output::new(peripherals.GPIO22, Level::Low, OutputConfig::default());

    // Perform display reset sequence
    cs_output.set_low();
    delay.delay_millis(50);
    rst.set_low();
    delay.delay_millis(50);
    rst.set_high();
    delay.delay_millis(50);

    // Configure backlight PWM
    let bk_light = Output::new(peripherals.GPIO23, Level::Low, OutputConfig::default());
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

    channel0.set_duty(BACKLIGHT_DUTY).unwrap();

    // ========================================
    // SPI INTERFACE SETUP
    // ========================================
    println!("Setup SPI interface");
    let miso = peripherals.GPIO3;
    let mosi = peripherals.GPIO2;
    let sclk = peripherals.GPIO1;
    let dc = Output::new(peripherals.GPIO15, Level::Low, OutputConfig::default());

    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(Rate::from_mhz(80))
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_miso(miso)
    .with_mosi(mosi);

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs_output).unwrap();

    // ========================================
    // DISPLAY INITIALIZATION
    // ========================================
    let mut buffer = [0_u8; 512];
    let di = SpiInterface::new(spi_device, dc, &mut buffer);

    let mut display = Builder::new(ILI9341Rgb565, di)
        .reset_pin(rst)
        .display_offset(34, 0)
        .display_size(DISPLAY_WIDTH, DISPLAY_HEIGHT)
        .invert_colors(ColorInversion::Normal)
        .orientation(Orientation::new().flip_horizontal())
        .init(&mut delay)
        .unwrap();

    // Clear display and draw initial content
    display.clear(Rgb565::BLACK).unwrap();
    draw_smiley(&mut display).unwrap();

    // ========================================
    // SENSOR SETUP
    // ========================================

    // Setup text rendering style
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(Rgb565::WHITE)
        .background_color(Rgb565::BLACK)
        .build();

    // Configure ADC for battery voltage monitoring
    let mut adc1_config = AdcConfig::new();
    let mut vbat_pin = adc1_config.enable_pin(peripherals.GPIO0, Attenuation::_11dB);
    let mut vbat_adc1 = Adc::new(peripherals.ADC1, adc1_config);

    // Setup temperature sensor
    let temperature_sensor =
        tsens::TemperatureSensor::new(peripherals.TSENS, tsens::Config::default()).unwrap();

    // ========================================
    // MAIN APPLICATION LOOP
    // ========================================
    loop {
        delay.delay(Duration::from_secs(1));

        // Read temperature sensor
        let temp = temperature_sensor.get_temperature();
        let temp_str = format!("Temperature: {:.2} C", temp.to_celsius());

        // Read battery voltage via ADC
        let vbat_v: f32 = vbat_adc1.read_oneshot(&mut vbat_pin).unwrap() as f32 * VAL_TO_VOLT;
        let volt_str = format!("VBAT ADC: {:.2} V", vbat_v);

        // Update display with sensor readings
        Text::new(volt_str.as_str(), Point::new(20, 30), text_style)
            .draw(&mut display)
            .unwrap();
        Text::new(temp_str.as_str(), Point::new(20, 40), text_style)
            .draw(&mut display)
            .unwrap();
    }
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

    Ok(())
}
