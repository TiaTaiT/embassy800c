// main.rs
#![no_std]
#![no_main]

mod hardware;

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{AnyPin, Level, Output, Speed};
use embassy_stm32::Peri;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

use hardware::init;

// --- BLINK TASK ---
#[embassy_executor::task(pool_size = 2)]
async fn blink_task(pin: Peri<'static, AnyPin>, interval_ms: u64) {
    // Output::new works fine with Peri<'static, AnyPin>
    let mut led = Output::new(pin, Level::Low, Speed::Low);
    loop {
        led.toggle();
        Timer::after_millis(interval_ms).await;
    }
}

// --- MAIN ---
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut board = init();
    
    info!("Main started using separated hardware module.");

    // Spawn Blink Tasks
    spawner.spawn(blink_task(board.leds.led3, 500)).unwrap();
    spawner.spawn(blink_task(board.leds.led4, 200)).unwrap();

    loop {
        // ADC Read
        let val1 = board.analog_inputs.adc.read(&mut board.analog_inputs.alarm_in_1).await;
        let val2 = board.analog_inputs.adc.read(&mut board.analog_inputs.alarm_in_2).await;
        let val3 = board.analog_inputs.adc.read(&mut board.analog_inputs.alarm_in_3).await;

        let mv1 = (val1 as u32 * 3300) / 4095;
        let mv2 = (val2 as u32 * 3300) / 4095;
        let mv3 = (val3 as u32 * 3300) / 4095;

        info!("ADC [mV]: {}, {}, {}", mv1, mv2, mv3);

        board.sim800_control.sim800_enable.toggle();
        board.sim800_control.sim800_ttl.toggle();

        Timer::after_millis(1000).await;
    }
}