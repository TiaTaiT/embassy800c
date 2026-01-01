#![no_std]

use core::cell::RefCell;
use core::fmt::Write;
use core::str::from_utf8;

use embassy_stm32::mode::Async;
use embassy_stm32::usart::{UartRx, UartTx};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_sync::mutex::Mutex;
use embassy_time::{with_timeout, Duration, Timer};
use embedded_io_async::Read;
use heapless::{String, Vec};
use defmt::{info, warn, error, debug};

// --- Constants from your code ---
pub const MAX_PHONE_LENGTH: usize = 20;
pub const SIM800_LINE_BUFFER_SIZE: usize = 128;
pub const MAX_DTMF_LEN: usize = 32;

// --- Data Structures ---

#[derive(Clone, Debug, defmt::Format)] 
pub struct Sms {
    pub number: String<MAX_PHONE_LENGTH>,
    pub timestamp: String<20>,
    pub message: String<SIM800_LINE_BUFFER_SIZE>,
}

#[derive(Clone, Debug, defmt::Format)]
pub enum SimEvent {
    IncomingCall(String<MAX_PHONE_LENGTH>),
    CallEnded,
    SmsReceived(Sms),
    SmsMemoryFull,
    SystemReady,
}

#[derive(Clone, Copy, PartialEq, Debug, defmt::Format)]
pub enum CommandResult {
    Ok,
    Error,
    Prompt, // The ">" character
    Timeout,
}

// --- Internal Signals ---
// Used to notify the command-sender that a response arrived
static RESPONSE_SIGNAL: Signal<CriticalSectionRawMutex, CommandResult> = Signal::new();

// --- The Driver Structs ---

/// The public handle used by main to control the modem
pub struct Sim800<'a> {
    tx: Mutex<CriticalSectionRawMutex, UartTx<'a, Async>>,
    // We assume the event channel is passed to main separately, 
    // or we could wrap it here.
}

impl<'a> Sim800<'a> {
    pub fn new(tx: UartTx<'a, Async>) -> Self {
        Self {
            tx: Mutex::new(tx),
        }
    }

    // --- Helper: Send Raw Command and Wait for OK/ERROR ---
    async fn send_cmd_wait(&self, cmd: &str, timeout_ms: u64) -> CommandResult {
        // 1. Clear any old signals
        RESPONSE_SIGNAL.reset();

        // 2. Send Command
        {
            let mut tx = self.tx.lock().await;
            debug!("TX: {}", cmd.trim());
            let _ = tx.write(cmd.as_bytes()).await;
        }

        // 3. Wait for signal or timeout
        match with_timeout(Duration::from_millis(timeout_ms), RESPONSE_SIGNAL.wait()).await {
            Ok(res) => res,
            Err(_) => CommandResult::Timeout,
        }
    }

    // --- Initialization ---
    pub async fn init(&self) -> bool {
        info!("Initializing SIM800...");
        
        // Basic check
        if self.send_cmd_wait("AT\r\n", 1000).await != CommandResult::Ok {
            return false;
        }
        
        // Disable Echo
        self.send_cmd_wait("ATE0\r\n", 1000).await;
        
        // Full Error Messages
        self.send_cmd_wait("AT+CMEE=1\r\n", 1000).await;
        
        // Caller ID
        self.send_cmd_wait("AT+CLIP=1\r\n", 1000).await;
        
        // SMS Text Mode
        self.send_cmd_wait("AT+CMGF=1\r\n", 1000).await;
        
        // SMS Notification: buffer new SMS, notify with +CMT directly
        self.send_cmd_wait("AT+CNMI=2,2,0,0,0\r\n", 1000).await;

        info!("SIM800 Init Complete");
        true
    }

    // --- Sending SMS (Linear Logic!) ---
    pub async fn send_sms(&self, number: &str, message: &str) -> bool {
        info!("Sending SMS to {}", number);

        // 1. Send CMGS command
        let mut cmd: String<64> = String::new();
        let _ = write!(cmd, "AT+CMGS=\"{}\"\r\n", number);
        
        // 2. Expect '>' prompt
        let res = self.send_cmd_wait(&cmd, 5000).await;
        if res != CommandResult::Prompt {
            error!("Failed to get SMS prompt >. Got: {:?}", res);
            return false;
        }

        // 3. Send Body + Ctrl-Z
        RESPONSE_SIGNAL.reset(); // clear the Prompt signal
        {
            let mut tx = self.tx.lock().await;
            let _ = tx.write(message.as_bytes()).await;
            let _ = tx.write(&[0x1A]).await; // CTRL+Z
        }

        // 4. Wait for final OK (can take seconds)
        match with_timeout(Duration::from_secs(10), RESPONSE_SIGNAL.wait()).await {
            Ok(CommandResult::Ok) => {
                info!("SMS Sent Successfully");
                true
            },
            _ => {
                error!("SMS Send Failed");
                false
            }
        }
    }

    // --- Make Call ---
    pub async fn make_call(&self, number: &str) -> bool {
        info!("Calling {}", number);
        let mut cmd: String<64> = String::new();
        let _ = write!(cmd, "ATD{};\r\n", number);
        
        if self.send_cmd_wait(&cmd, 1000).await == CommandResult::Ok {
            // Note: ATD returns OK immediately, then audio starts.
            // Monitoring connection state usually requires parsing +CLCC or similar polling
            // handled in the RX loop, or logic here.
            true
        } else {
            false
        }
    }
    
    pub async fn hang_up(&self) {
        self.send_cmd_wait("AT+CHUP\r\n", 1000).await;
    }
}

// --- The Background Reader Task ---

// This task consumes the Rx part of the UART.
// It parses every incoming byte.
// 1. If it's a Command Response (OK, ERROR, >) -> Signal the Controller
// 2. If it's an Event (+CMT, RING) -> Push to Event Channel
pub async fn rx_runner(
    mut rx: UartRx<'static, Async>, 
    event_channel: &Channel<CriticalSectionRawMutex, SimEvent, 4>
) {
    let mut dma_buf = [0u8; 512];
    let mut ring = rx.into_ring_buffered(&mut dma_buf);
    
    let mut line_buf = [0u8; SIM800_LINE_BUFFER_SIZE];
    let mut pos = 0;

    // State for multi-line parsing (like SMS content)
    let mut expecting_sms_body = false;
    let mut pending_sms_header: Option<(String<MAX_PHONE_LENGTH>, String<20>)> = None;

    loop {
        let mut byte_buf = [0u8; 1];
        if let Err(_) = ring.read(&mut byte_buf).await {
            continue;
        }
        let b = byte_buf[0];

        // --- Handle Prompt '>' specially ---
        // It often comes without a newline when asking for SMS body
        if b == b'>' && pos == 0 {
             // If we just got a '>', it's likely the SMS prompt.
             // We can signal immediately.
             RESPONSE_SIGNAL.signal(CommandResult::Prompt);
             continue;
        }

        // --- Line Assembly ---
        if b == b'\n' {
            // Process Line
            let len = if pos > 0 && line_buf[pos-1] == b'\r' { pos - 1 } else { pos };
            
            if let Ok(line) = from_utf8(&line_buf[..len]) {
                let clean = line.trim();
                if !clean.is_empty() {
                    debug!("RX: {}", clean);
                    
                    // 1. Is it SMS Body?
                    if expecting_sms_body {
                         if let Some((num, ts)) = pending_sms_header.take() {
                             // Create the SMS
                             let mut msg = String::new();
                             let _ = msg.push_str(clean); // Truncates if too long
                             
                             let event = SimEvent::SmsReceived(Sms {
                                 number: num,
                                 timestamp: ts,
                                 message: msg
                             });
                             let _ = event_channel.try_send(event);
                         }
                         expecting_sms_body = false;
                    } 
                    // 2. Is it a Command Response?
                    else if clean == "OK" {
                        RESPONSE_SIGNAL.signal(CommandResult::Ok);
                    } else if clean == "ERROR" {
                        RESPONSE_SIGNAL.signal(CommandResult::Error);
                    } 
                    // 3. Is it an Unsolicited Event?
                    else if clean == "RING" {
                        // We don't have the number yet usually, unless +CLIP comes
                        // We can signal generic call or wait for +CLIP
                    } else if clean.starts_with("+CLIP:") {
                        let num = parse_quoted(clean, 0); // Helper to get number
                        let _ = event_channel.try_send(SimEvent::IncomingCall(num));
                    } else if clean.starts_with("NO CARRIER") {
                         let _ = event_channel.try_send(SimEvent::CallEnded);
                    } else if clean.starts_with("+CMT:") {
                        // Format: +CMT: "+12345","","24/01/01,12:00:00+00"
                        // The *next* line will be the body.
                        let num = parse_quoted(clean, 0);
                        let ts = parse_quoted(clean, 2); // roughly 3rd quote group?
                        pending_sms_header = Some((num, ts));
                        expecting_sms_body = true;
                    } else if clean.starts_with("Call Ready") {
                        let _ = event_channel.try_send(SimEvent::SystemReady);
                    }
                }
            }
            pos = 0; // Reset buffer
        } else {
            if pos < SIM800_LINE_BUFFER_SIZE {
                line_buf[pos] = b;
                pos += 1;
            } else {
                // Overflow protection
                pos = 0; 
            }
        }
    }
}

// --- Helpers ---

// Very naive parser to extract string between quotes.
// `idx` indicates which quoted pair to extract (0 for first, 1 for second...)
fn parse_quoted<const N: usize>(input: &str, target_idx: usize) -> String<N> {
    let mut current_idx = 0;
    let mut in_quote = false;
    let mut result = String::new();
    
    for c in input.chars() {
        if c == '"' {
            if in_quote {
                // End of a quoted section
                if current_idx == target_idx {
                    return result;
                }
                current_idx += 1;
                in_quote = false;
            } else {
                // Start of a quoted section
                in_quote = true;
                if current_idx == target_idx {
                    result.clear();
                }
            }
        } else if in_quote && current_idx == target_idx {
            let _ = result.push(c);
        }
    }
    result
}