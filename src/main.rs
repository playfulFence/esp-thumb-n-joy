#![no_std]
#![no_main]


use embedded_hal::blocking::delay;
#[cfg(feature="esp32")]
use esp32_hal as hal;
#[cfg(feature="esp32s2")]
use esp32s2_hal as hal;
#[cfg(feature="esp32s3")]
use esp32s3_hal as hal;
#[cfg(feature="esp32c3")]
use esp32c3_hal as hal;

use hal::{
    adc::{AdcConfig, Attenuation, ADC, ADC1},
    analog::SarAdcExt,
    clock::ClockControl,
    pac::Peripherals,
    gpio_types::*,
    gpio::*,
    prelude::*,
    spi,
    timer::TimerGroup,
    Rtc,
    IO,
    Delay,
};

/* Display and graphics */
#[cfg(feature = "ili9341")]
use ili9341::{DisplaySize240x320, Ili9341, Orientation};
#[cfg(feature = "st7789")]
use st7789::*;

/* Display and graphics stuff */
use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::*;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::*;
use embedded_graphics::text::*;
use embedded_graphics::image::Image;
use embedded_graphics::geometry::*;
use embedded_graphics::draw_target::DrawTarget;
use embedded_hal;

/* Fonts */
use profont::{PROFONT_24_POINT, PROFONT_18_POINT};



#[cfg(feature="xtensa-lx-rt")]
use xtensa_lx_rt::entry;
#[cfg(feature="riscv-rt")]
use riscv_rt::entry;

use esp_println::println;
use esp_backtrace as _;


/* Some stuff for correct orientation and color on ILI9341 */
pub enum KalugaOrientation {
    Portrait,
    PortraitFlipped,
    Landscape,
    LandscapeVericallyFlipped,
    LandscapeFlipped,
}

impl ili9341::Mode for KalugaOrientation {
    fn mode(&self) -> u8 {
        match self {
            Self::Portrait => 0,
            Self::LandscapeVericallyFlipped => 0x20,
            Self::Landscape => 0x20 | 0x40,
            Self::PortraitFlipped => 0x80 | 0x40,
            Self::LandscapeFlipped => 0x80 | 0x20,
        }
    }

    fn is_landscape(&self) -> bool {
        matches!(self, Self::Landscape | Self::LandscapeFlipped | Self::LandscapeVericallyFlipped)
    }
}



/* Debouncing algorithm */

#[derive(Copy, Clone, PartialEq)]
pub enum Event {
    Pressed,
    Released,
    Nothing,
}
pub struct Button<T> {
    button: T,
    pressed: bool,
}
impl<T: ::embedded_hal::digital::v2::InputPin<Error = core::convert::Infallible>> Button<T> {
    pub fn new(button: T) -> Self {
        Button {
            button,
            pressed: true,
        }
    }
    pub fn check(&mut self){
        self.pressed = !self.button.is_low().unwrap();
    }

    pub fn poll(&mut self, delay :&mut Delay) -> Event {
        let pressed_now = !self.button.is_low().unwrap();
        if !self.pressed  &&  pressed_now
        {
            delay.delay_ms(30 as u32);
            self.check();
            if !self.button.is_low().unwrap() {
                Event::Pressed
            }
            else {
                Event::Nothing
            }
        }
        else if self.pressed && !pressed_now{
            delay.delay_ms(30 as u32);
            self.check();
            if self.button.is_low().unwrap()
            {
                Event::Released
            }
            else {
                Event::Nothing
            }
        }
        else{
            Event::Nothing
        }
        
    }
}

 

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take().unwrap();

    #[cfg(any(feature = "esp32"))]
    let mut system = peripherals.DPORT.split();
    #[cfg(any(feature = "esp32s2", feature = "esp32s3", feature = "esp32c3"))]
    let mut system = peripherals.SYSTEM.split();

    let mut clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    // Disable the RTC and TIMG watchdog timers
    let mut rtc = Rtc::new(peripherals.RTC_CNTL);
    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    let mut wdt0 = timer_group0.wdt;
    let timer_group1 = TimerGroup::new(peripherals.TIMG1, &clocks);
    let mut wdt1 = timer_group1.wdt;
    
    rtc.rwdt.disable();
    wdt0.disable();
    wdt1.disable();


    println!("About to initialize the SPI LED driver ILI9341");
    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    

    // /* Set corresponding pins */
    // #[cfg(feature = "esp32")]
    // let mosi = io.pins.gpio23;
    // #[cfg(any(feature = "esp32s2", feature = "esp32s3"))]
    // let mosi = io.pins.gpio7;
    // #[cfg(feature = "esp32c3")]
    // let mosi = io.pins.gpio7;

    // #[cfg(feature = "esp32")]
    // let cs = io.pins.gpio22;
    // #[cfg(any(feature = "esp32s2", feature = "esp32s3"))]
    // let cs = io.pins.gpio5;
    // #[cfg(feature = "esp32c3")]
    // let cs = io.pins.gpio20;

    // #[cfg(feature = "esp32")]
    // let rst = io.pins.gpio18;
    // #[cfg(any(feature = "esp32s2", feature = "esp32s3"))]
    // let rst = io.pins.gpio18;
    // #[cfg(feature = "esp32c3")]
    // let rst = io.pins.gpio3;

    // #[cfg(feature = "esp32")]
    // let dc = io.pins.gpio21;
    // #[cfg(any(feature = "esp32s2", feature = "esp32s3"))]
    // let dc = io.pins.gpio4;
    // #[cfg(feature = "esp32c3")] 
    // let dc = io.pins.gpio21;

    // #[cfg(feature = "esp32")]
    // let sck = io.pins.gpio19;
    // #[cfg(any(feature = "esp32s2", feature = "esp32s3"))]
    // let sck = io.pins.gpio6;
    // #[cfg(feature = "esp32c3")]
    // let sck = io.pins.gpio6;

    // #[cfg(feature = "esp32")]
    // let miso = io.pins.gpio25;
    // #[cfg(any(feature = "esp32s2", feature = "esp32s3"))]
    // let miso = io.pins.gpio12;
    // #[cfg(feature = "esp32c3")]
    // let miso = io.pins.gpio8;

    // #[cfg(feature = "esp32")]
    // let mut backlight = io.pins.gpio5.into_push_pull_output();
    // #[cfg(any(feature = "esp32s2", feature = "esp32s3"))]
    // let mut backlight = io.pins.gpio9.into_push_pull_output();
    // #[cfg(feature = "esp32c3")]
    // let mut backlight = io.pins.gpio0.into_push_pull_output();

    // // /* Then set backlight (set_low() - display lights up when signal is in 0, set_high() - opposite case(for example.)) */
    // let mut backlight = backlight.into_push_pull_output();
    // backlight.set_high().unwrap();


    // // /* Configure SPI */
    // #[cfg(feature = "esp32")]
    // let spi = spi::Spi::new(
    //     peripherals.SPI3,
    //     sck,
    //     mosi,
    //     miso,
    //     cs,
    //     100u32.MHz(),
    //     spi::SpiMode::Mode0,
    //     &mut system.peripheral_clock_control,
    //     &mut clocks,
    // );
    // #[cfg(any(feature = "esp32s2", feature = "esp32s3"))]
    // let spi = spi::Spi::new(
    //     peripherals.SPI2,
    //     sck,
    //     mosi,
    //     miso,
    //     cs,
    //     80u32.MHz(),
    //     spi::SpiMode::Mode0,
    //     &mut system.peripheral_clock_control,
    //     &mut clocks,
    // );
    // #[cfg(feature = "esp32c3")]
    // let spi = spi::Spi::new(
    //     peripherals.SPI2,
    //     sck,
    //     mosi,
    //     miso,
    //     cs,
    //     80u32.MHz(),
    //     spi::SpiMode::Mode0,
    //     &mut system.peripheral_clock_control,
    //     &mut clocks,
    // );

    // let di = SPIInterfaceNoCS::new(spi, dc.into_push_pull_output());
    // let reset = rst.into_push_pull_output();
    let mut delay = Delay::new(&clocks);
    // #[cfg(feature = "ili9341")]
    // let mut display = Ili9341::new(di, reset, &mut delay, KalugaOrientation::Landscape, DisplaySize240x320).unwrap();
    // #[cfg(feature = "st7789")]
    // let mut display = st7789::ST7789::new(di, reset, 240, 240);

    // /* st7789 display requires additional init functions */

    // #[cfg(feature = "st7789")]
    // display.init(&mut delay).unwrap();
    // #[cfg(feature = "st7789")]
    // display.set_orientation(st7789::Orientation::Portrait).unwrap();
    

    // println!("Initialized");

    // display.clear(Rgb565::WHITE).unwrap();
    let analog = peripherals.APB_SARADC.split();

    let mut adc1_config = AdcConfig::new();
    
    let mut joystick_select = Button::new(io.pins.gpio4.into_pull_up_input()); // MENU button on esp32-s3-usb-otg (low level when pressed)
    let mut y_axis = adc1_config.enable_pin(io.pins.gpio1.into_analog(), Attenuation::Attenuation11dB);
    let mut x_axis = adc1_config.enable_pin(io.pins.gpio0.into_analog(), Attenuation::Attenuation11dB);

    let mut adc1 = ADC::<ADC1>::adc(
        &mut system.peripheral_clock_control,
        analog.adc1,
        adc1_config,
    )
    .unwrap();

    /* Counters for number of pushes of each button.
       Just to verify, that debouncing works correctly  */
    let mut select_cnt = 0;
    let mut down_cnt = 0; 
    let mut up_cnt   = 0;
        
    println!("Joystick calibration in 3");
    delay.delay_ms(1000 as u32);
    println!("Joystick calibration in 2");
    delay.delay_ms(1000 as u32);
    println!("Joystick calibration in 1");
    delay.delay_ms(1000 as u32);

    // let mut y_axis_zero : u16 = nb::block!(adc1.read(&mut y_axis)).unwrap();
    // let mut x_axis_zero : u16 = nb::block!(adc1.read(&mut x_axis)).unwrap();

    let mut y_axis_actual :u16 = 0;
    let mut x_axis_actual :u16 = 0;

    //println!("Y_AXIS_ZERO : {} \tY_AXIS : {}", y_axis_zero, y_axis.read_input() as u32);
    
    loop {
        y_axis_actual = nb::block!(adc1.read(&mut y_axis)).unwrap();
        x_axis_actual = nb::block!(adc1.read(&mut x_axis)).unwrap();
        println!("X_AXIS : {}\tY_AXIS : {}", x_axis_actual, y_axis_actual );
    }
}
