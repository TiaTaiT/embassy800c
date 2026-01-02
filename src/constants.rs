// /src/constants.rs
pub const USE_SMS: bool = false;

pub const LOW_INTRUSION_THRESHOLD: u16 = 1000;
pub const HIGH_INTRUSION_THRESHOLD: u16 = 1500;

pub const ALARMS_CHANNELS_AMOUNT: usize = 3;
pub const ALARMS_STACK_DEPTH: usize = 3;
pub const ALARMS_BUFFER_SIZE: usize = 256;
pub const ALARMS_MESSAGE_STRING_LENGTH: usize = 3;

pub const INIT_SIM800_DELAY_SECONDS: u32 = 6;
pub const ALIVE_PERIOD_MINUTES: i32 = 120;
pub const SYSTEM_MONITOR_PERIOD_HOURS: u32 = 12;

pub const SMS_PREFIX: &str = "PPP";
pub const ONLINE_SIGNAL: &str = "*";
pub const CONFIRMATION_SIGNAL: &str = "#";
pub const ERROR_SIGNAL: &str = "0";
pub const DTMF_PACKET_LENGTH: usize = 3;

pub const MAX_PHONE_LENGTH: usize = 16;

pub const SIM800_LINE_BUFFER_SIZE: usize = 64;
pub const MAXIMUM_DTMF_BUFFER_SIZE: usize = 16;
pub const MAXIMUM_SIM800_LINE_COUNT: usize = 8;
pub const MAXIMUM_INCOMING_SMS_BUFFER_SIZE: usize = 8;

pub const SIM800_RX_BUFFER_SIZE: usize = 256;

/// The system clock frequency (in Hertz).
pub const SYSCLK_HZ:    u32 = 16_000_000;
/// How often `check_intrusion` to run (in Hertz).
pub const MONOTONIC_TICK_HZ: u32 = 10;

pub const ALARM_MANAGER_TICK_MINUTES: u32 = 1;