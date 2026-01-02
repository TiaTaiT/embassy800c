// /src/main.rs
#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use heapless::String;

mod constants;
mod hardware;
mod alarms_handler;
mod rtc;
mod sim800;
mod gsm_time_converter;
mod date_converter;
mod phone_book;
mod custom_strings;

use crate::constants::*;
use crate::hardware::{AnalogInputs, AlarmOutputs};
use crate::alarms_handler::{AlarmStack, AlarmTracker};
use crate::rtc::RtcControl;
use crate::sim800::{Command, Sim800Driver, SimEvent};

// --- Global Signals/Channels ---
static CMD_CHANNEL: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();
static EVENT_CHANNEL: Channel<CriticalSectionRawMutex, SimEvent, 4> = Channel::new();

// Shared State
struct SystemState {
    alarm_stack: AlarmStack,
    alive_countdown: i32,
    cancellation_token: u32,
}

static STATE: Mutex<CriticalSectionRawMutex, SystemState> = Mutex::new(SystemState {
    // Corrected to use the public `new` constructor
    alarm_stack: AlarmStack::new(), 
    alive_countdown: 0,
    cancellation_token: 0,
});

static RTC: Mutex<CriticalSectionRawMutex, Option<RtcControl>> = Mutex::new(None);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let board = hardware::init();
    
    // RTC Init
    {
        let rtc_ctrl = RtcControl::init();
        let mut rtc_lock = RTC.lock().await;
        *rtc_lock = Some(rtc_ctrl);
    }

    info!("Starting Embassy800c...");

    // Spawn Tasks
    spawner.spawn(sim800_task(board.uart2_tx, board.uart2_rx, board.sim800_control)).unwrap();
    spawner.spawn(adc_monitor_task(board.analog_inputs)).unwrap();
    spawner.spawn(logic_task(board.alarm_outputs)).unwrap();
    spawner.spawn(system_monitor_task()).unwrap();
}

#[embassy_executor::task]
async fn sim800_task(tx: hardware::Uart2Tx, rx: hardware::Uart2Rx, control: hardware::Sim800Control) {
    let mut driver = Sim800Driver::new(tx, rx, control);
    
    // Command Init
    CMD_CHANNEL.send(Command::Init).await;

    driver.run(CMD_CHANNEL.receiver(), EVENT_CHANNEL.sender()).await;
}

#[embassy_executor::task]
async fn adc_monitor_task(mut inputs: AnalogInputs) {
    let mut adc = inputs.adc;
    loop {
        let val1 = adc.read(&mut inputs.alarm_in_1).await;
        let val2 = adc.read(&mut inputs.alarm_in_2).await;
        let val3 = adc.read(&mut inputs.alarm_in_3).await;

        let bools = [
            val1 > LOW_INTRUSION_THRESHOLD && val1 < HIGH_INTRUSION_THRESHOLD,
            val2 > LOW_INTRUSION_THRESHOLD && val2 < HIGH_INTRUSION_THRESHOLD,
            val3 > LOW_INTRUSION_THRESHOLD && val3 < HIGH_INTRUSION_THRESHOLD,
        ];

        {
            let mut state = STATE.lock().await;
            state.alarm_stack.push(&bools);
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn logic_task(mut outputs: AlarmOutputs) {
    loop {
        // 1. Process Events from SIM800 (SMS/Calls)
        if let Ok(event) = EVENT_CHANNEL.try_receive() {
            match event {
                SimEvent::SmsReceived { message, .. } => {
                    if let Some(alarm_str) = custom_strings::extract_before_delimiter(&message, ";") {
                         if alarm_str.len() == ALARMS_MESSAGE_STRING_LENGTH {
                             play_alarms(&mut outputs, alarm_str).await;
                         }
                    }
                },
                SimEvent::CallReceived { number } => {
                    CMD_CHANNEL.send(Command::HandleIncomingCall { phone_number: number }).await;
                },
                SimEvent::DtmfReceived(c) => {
                     info!("DTMF: {}", c);
                }
                _ => {}
            }
        }

        // 2. Process Alarm Stack Logic (Sending Alarms)
        {
            let mut state = STATE.lock().await;
            let tick = state.alive_countdown <= 0;
            if state.alarm_stack.has_changes() || tick {
                let bits = state.alarm_stack.export_bits();
                let str_stack: String<DTMF_PACKET_LENGTH> = bits.iter().collect();
                
                state.alive_countdown = ALIVE_PERIOD_MINUTES + 1;

                if USE_SMS {
                     let mut msg: String<SIM800_LINE_BUFFER_SIZE> = String::new();
                     use core::fmt::Write;
                     let _ = write!(msg, "{}{}", SMS_PREFIX, str_stack);
                     CMD_CHANNEL.send(Command::SendAlarmSms { message: msg }).await;
                } else {
                     CMD_CHANNEL.send(Command::CallAlarmWithDtmf { dtmf: str_stack }).await;
                }
            }
            state.alive_countdown -= 1;
        }

        Timer::after(Duration::from_secs(60)).await;
    }
}

async fn play_alarms(outputs: &mut AlarmOutputs, alarm_str: &str) {
    info!("Playing alarms: {}", alarm_str);
    outputs.alarm_out_1.set_high();
    Timer::after(Duration::from_secs(3)).await;
    outputs.alarm_out_1.set_low();
}

#[embassy_executor::task]
async fn system_monitor_task() {
    loop {
        Timer::after(Duration::from_secs(SYSTEM_MONITOR_PERIOD_HOURS as u64 * 3600)).await;
        CMD_CHANNEL.send(Command::UpdateTime).await;
    }
}