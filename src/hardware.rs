// hardware.rs
use embassy_stm32::adc::{Adc, SampleTime};
use embassy_stm32::gpio::{AnyPin, Level, Output, Speed};
use embassy_stm32::mode::Async;
use embassy_stm32::peripherals::{self, ADC1, PA4, PA5, PA6};
use embassy_stm32::rcc::{Hse, HseMode, Pll, PllMul, PllPreDiv, PllSource, Sysclk};
use embassy_stm32::time::Hertz;
use embassy_stm32::usart::{Config as UartConfig, Uart};
use embassy_stm32::{adc, bind_interrupts, usart, Config, Peri};
use defmt::info;

bind_interrupts!(pub struct Irqs {
    ADC1_COMP => adc::InterruptHandler<peripherals::ADC1>;
    USART1 => usart::InterruptHandler<peripherals::USART1>;
    USART2 => usart::InterruptHandler<peripherals::USART2>;
});

// Correct Type Aliases for Async UART
pub type Uart1 = Uart<'static, Async>;
pub type Uart2 = Uart<'static, Async>;
pub type Adc1 = Adc<'static, ADC1>;

pub struct AnalogInputs {
    pub alarm_in_1: Peri<'static, PA4>,
    pub alarm_in_2: Peri<'static, PA5>,
    pub alarm_in_3: Peri<'static, PA6>,
    pub adc: Adc1,
}

pub struct Leds {
    pub led3: Peri<'static, AnyPin>,
    pub led4: Peri<'static, AnyPin>,
}

pub struct AlarmOutputs {
    pub alarm_out_1: Output<'static>,
    pub alarm_out_2: Output<'static>,
    pub alarm_out_3: Output<'static>,
}

pub struct Sim800Control {
    pub sim800_enable: Output<'static>,
    pub sim800_ttl: Output<'static>,
}

pub struct Board {
    pub analog_inputs: AnalogInputs, 
    pub alarm_outputs: AlarmOutputs,
    pub uart1: Uart1,
    pub uart2: Uart2,
    pub leds: Leds,
    pub sim800_control: Sim800Control,
    pub _alarm_pullup: Output<'static>,
}

pub fn init() -> Board {
    // 1. Clock Configuration
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: Hertz::mhz(8),
        mode: HseMode::Bypass, 
    });
    config.rcc.pll = Some(Pll {
        src: PllSource::HSE,
        prediv: PllPreDiv::DIV1,
        mul: PllMul::MUL6,
    });
    config.rcc.sys = Sysclk::PLL1_P;

    let p = embassy_stm32::init(config);
    info!("Hardware initialized! Clocked at 48MHz");

    // 2. Additional Outputs
    let alarm_pullup = Output::new(p.PA7, Level::High, Speed::Low);
    let alarm_out_1 = Output::new(p.PB3, Level::High, Speed::Low);
    let alarm_out_2 = Output::new(p.PB4, Level::High, Speed::Low);
    let alarm_out_3 = Output::new(p.PB5, Level::High, Speed::Low);
    let out_pc6 = Output::new(p.PC6, Level::Low, Speed::Low);
    let out_pc7 = Output::new(p.PC7, Level::Low, Speed::Low);

    // 3. USART1
    let mut config_u1 = UartConfig::default();
    config_u1.baudrate = 115200;
    let uart1 = Uart::new(
        p.USART1,
        p.PA10, p.PA9,
        Irqs,
        p.DMA1_CH2, p.DMA1_CH3,
        config_u1,
    ).unwrap();

    // 4. USART2
    let mut config_u2 = UartConfig::default();
    config_u2.baudrate = 9600;
    let uart2 = Uart::new(
        p.USART2,
        p.PA3, p.PA2,
        Irqs,
        p.DMA1_CH4, p.DMA1_CH5,
        config_u2,
    ).unwrap();

    // 5. ADC
    let mut adc = Adc::new(p.ADC1, Irqs);
    adc.set_sample_time(SampleTime::CYCLES71_5);

    let analog_inputs = AnalogInputs {
        alarm_in_1: p.PA4,
        alarm_in_2: p.PA5,
        alarm_in_3: p.PA6,
        adc,
    };

    let alarm_outputs = AlarmOutputs {
        alarm_out_1,
        alarm_out_2,
        alarm_out_3,
    };

    let leds = Leds {
        led3: p.PC8.into(),
        led4: p.PC9.into(),
    };

    let sim800_control = Sim800Control {
        sim800_enable: out_pc6,
        sim800_ttl: out_pc7,
    };

    Board {
        analog_inputs,
        alarm_outputs,
        uart1,
        uart2,
        leds,
        sim800_control,
        _alarm_pullup: alarm_pullup,
    }
}