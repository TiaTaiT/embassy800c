// /src/main.rs
#![no_std]
#![no_main]

mod hardware;
mod sim800; // The file above

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{AnyPin, Level, Output, Speed};
use embassy_stm32::Peri;
use embassy_time::{Timer, Duration};
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

use {defmt_rtt as _, panic_probe as _};

use hardware::init;
use sim800::{Sim800, SimEvent, rx_runner};

// Create a static channel for Events coming from the SIM800
static EVENT_CHANNEL: Channel<CriticalSectionRawMutex, SimEvent, 4> = Channel::new();

#[embassy_executor::task]
async fn sim800_bg_task(rx: embassy_stm32::usart::UartRx<'static, embassy_stm32::mode::Async>) {
    // This function never returns. It handles the parsing.
    rx_runner(rx, &EVENT_CHANNEL).await;
}

#[embassy_executor::task(pool_size = 2)]
async fn blink_task(pin: Peri<'static, AnyPin>, interval_ms: u64) {
    let mut led = Output::new(pin, Level::Low, Speed::Low);
    loop {
        led.toggle();
        Timer::after_millis(interval_ms).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut board = init();
    
    info!("Starting...");

    // 1. Spawn the Background Reader
    spawner.spawn(sim800_bg_task(board.uart2_rx)).unwrap();

    // 2. Create the Controller (Owns TX)
    let modem = Sim800::new(board.uart2_tx);

    // 3. Initialize Modem (Linear code!)
    if modem.init().await {
        info!("Modem Initialized & Ready");
    } else {
        error!("Modem Init Failed");
    }

    // 4. Send a test SMS?
    // modem.send_sms("+1234567890", "Hello from Embassy!").await;

    loop {
        // We select between user actions and incoming events
        use embassy_futures::select::{select, Either};

        // Example: Wait for an event OR wait 10 seconds to blink
        match select(
            EVENT_CHANNEL.receive(),
            Timer::after(Duration::from_secs(10))
        ).await {
            Either::First(event) => {
                match event {
                    SimEvent::IncomingCall(num) => {
                        info!("Incoming call from: {}", num);
                        // Logic to auto-answer or hangup
                        // modem.make_call(...); // or hangup
                        modem.hang_up().await;
                    },
                    SimEvent::SmsReceived(sms) => {
                        info!("SMS from {}: {}", sms.number, sms.message);
                        // Reply?
                        // modem.send_sms(&sms.number, "Got it!").await;
                    },
                    SimEvent::CallEnded => info!("Call Ended"),
                    SimEvent::SystemReady => info!("System became ready"),
                    _ => {}
                }
            },
            Either::Second(_) => {
                info!("Heartbeat tick...");
                // Periodically check status or send AT?
            }
        }
    }
}