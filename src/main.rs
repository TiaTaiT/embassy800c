// /src/main.rs
#![no_std]
#![no_main]

use defmt::{info, warn};
use defmt_rtt as _;
use embassy_stm32::adc::SampleTime;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_futures::select::{select3, Either3};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Instant, Timer};
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
}

static STATE: Mutex<CriticalSectionRawMutex, SystemState> = Mutex::new(SystemState {
    alarm_stack: AlarmStack::new(), 
    alive_countdown: 0,
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
    CMD_CHANNEL.send(Command::Init).await;
    // Request time update immediately after initialization
    CMD_CHANNEL.send(Command::UpdateTime).await; 
    driver.run(CMD_CHANNEL.receiver(), EVENT_CHANNEL.sender()).await;
}

#[embassy_executor::task]
async fn adc_monitor_task(mut inputs: AnalogInputs) {
    let mut adc = inputs.adc;
    loop {
		let val1 = adc.read(&mut inputs.alarm_in_1, SampleTime::CYCLES71_5).await;
        let val2 = adc.read(&mut inputs.alarm_in_2, SampleTime::CYCLES71_5).await;
        let val3 = adc.read(&mut inputs.alarm_in_3, SampleTime::CYCLES71_5).await;

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
    let mut watchdog_deadline: Option<Instant> = None;
    let mut dtmf_buffer = String::<DTMF_PACKET_LENGTH>::new();
    
    // Sender logic timer
    let mut next_sender_tick = Instant::now() + Duration::from_secs(60);

    loop {
        // Prepare Futures
        
        // 1. Watchdog Future
        let watchdog_fut = async {
            if let Some(deadline) = watchdog_deadline {
                Timer::at(deadline).await;
                true
            } else {
                core::future::pending::<bool>().await
            }
        };

        // 2. Sender Tick Future
        let sender_fut = Timer::at(next_sender_tick);

        // 3. Event Future
        let event_fut = EVENT_CHANNEL.receive();

        // Wait for any of the 3
        match select3(event_fut, sender_fut, watchdog_fut).await {
            // --- CASE 1: SIM800 EVENT RECEIVED ---
            Either3::First(event) => {
                match event {
                    SimEvent::SmsReceived { message, .. } => {
                        if let Some(alarm_str) = custom_strings::extract_before_delimiter(&message, ";") {
                             if alarm_str.len() == ALARMS_MESSAGE_STRING_LENGTH {
                                 play_received_alarms(&mut outputs, alarm_str).await;
                                 watchdog_deadline = Some(Instant::now() + Duration::from_secs(255 * 60));
                             }
                        }
                    },
                    SimEvent::DtmfReceived(c) => {
                        if dtmf_buffer.push(c).is_ok() {
                            info!("DTMF Buffer: {}", dtmf_buffer.as_str());
                            if dtmf_buffer.len() == DTMF_PACKET_LENGTH {
                                play_received_alarms(&mut outputs, &dtmf_buffer).await;
                                watchdog_deadline = Some(Instant::now() + Duration::from_secs(255 * 60));
                                dtmf_buffer.clear();
                            }
                        }
                    },
                    SimEvent::CallEnded => {
                        dtmf_buffer.clear();
                    },
                    SimEvent::CallReceived { number } => {
                        CMD_CHANNEL.send(Command::HandleIncomingCall { phone_number: number }).await;
                    },
                    SimEvent::CallExecuted(success) => {
                        if success { info!("Alarm Call Confirmed by Remote"); }
                        else { warn!("Alarm Call Failed"); }
                    },
                    SimEvent::TimeReceived(time) => {
                         info!("Updating RTC...");
                         let mut rtc = RTC.lock().await;
                         if let Some(ref mut rtc_ctrl) = *rtc {
                             rtc_ctrl.set_time(time);
                         }
                         info!("RTC was updated.");
                    }
                }
            },

            // --- CASE 2: SENDER LOGIC TICK (Every 60s) ---
            Either3::Second(_) => {
                next_sender_tick += Duration::from_secs(60);
                
                let mut pending_dtmf: Option<String<DTMF_PACKET_LENGTH>> = None;
                let mut pending_sms: Option<String<SIM800_LINE_BUFFER_SIZE>> = None;
                let mut is_sms = false;

                // Scope lock
                {
                    let mut state = STATE.lock().await;
                    let tick = state.alive_countdown <= 0;
                    
                    if state.alarm_stack.has_changes() || tick {
                        let bits = state.alarm_stack.export_bits();
                        let str_stack: String<DTMF_PACKET_LENGTH> = bits.iter().collect();
                        
                        state.alive_countdown = ALIVE_PERIOD_MINUTES + 1;

                        if USE_SMS {
                             let time_buf = {
                                 let rtc = RTC.lock().await;
                                 // Use 'ref' instead of 'ref mut' because get_time is immutable
                                 if let Some(ref rtc_ctrl) = *rtc {
                                     let t = rtc_ctrl.get_time();
                                     crate::date_converter::format_gsm_time(&t)
                                 } else {
                                     crate::date_converter::format_gsm_time(&crate::rtc::GsmTime { 
                                         year:0, month:0, day:0, hour:0, minute:0, second:0 
                                     })
                                 }
                             };

                             let mut msg = String::<SIM800_LINE_BUFFER_SIZE>::new();
                             use core::fmt::Write;
                             let _ = write!(msg, "{}{}{}{}{}", SMS_PREFIX, SMS_DIVIDER, str_stack, SMS_DIVIDER, time_buf.as_str());
                             pending_sms = Some(msg);
                             is_sms = true;
                        } else {
                             pending_dtmf = Some(str_stack);
                        }
                    }
                    if !tick {
                        state.alive_countdown -= 1;
                    }
                }

                if is_sms {
                    if let Some(msg) = pending_sms {
                        CMD_CHANNEL.send(Command::SendAlarmSms { message: msg }).await;
                    }
                } else if let Some(dtmf) = pending_dtmf {
                    info!("Sending Alarm Report: {}", dtmf.as_str());
                    CMD_CHANNEL.send(Command::CallAlarmWithDtmf { dtmf }).await;
                }
            },

            // --- CASE 3: WATCHDOG TIMEOUT ---
            Either3::Third(_) => {
                info!("Watchdog 4.5h expired. Resetting relays to Low.");
                outputs.alarm_out_1.set_low();
                outputs.alarm_out_2.set_low();
                outputs.alarm_out_3.set_low();
                watchdog_deadline = None;
            }
        }
    }
}

async fn play_received_alarms(outputs: &mut AlarmOutputs, alarm_str: &str) {
    info!("Playing received alarms: {}", alarm_str);
    
    let mut alarm_chars = ['\0'; ALARMS_MESSAGE_STRING_LENGTH];
    for (i, c) in alarm_str.chars().take(ALARMS_MESSAGE_STRING_LENGTH).enumerate() {
        alarm_chars[i] = c;
    }

    let mut temp_stack = AlarmStack::new();
    temp_stack.import_bits(alarm_chars);
    let matrix = temp_stack.get_stack_view();

    for row in matrix.iter() {
        if row[0] { outputs.alarm_out_1.set_high(); } else { outputs.alarm_out_1.set_low(); }
        if row[1] { outputs.alarm_out_2.set_high(); } else { outputs.alarm_out_2.set_low(); }
        if row[2] { outputs.alarm_out_3.set_high(); } else { outputs.alarm_out_3.set_low(); }
        
        Timer::after(Duration::from_secs(3)).await;
    }

    info!("Alarm playback finished. Relays holding last state.");
}

#[embassy_executor::task]
async fn system_monitor_task() {
    loop {
        Timer::after(Duration::from_secs(SYSTEM_MONITOR_PERIOD_HOURS as u64 * 3600)).await;
        CMD_CHANNEL.send(Command::UpdateTime).await;
    }
}