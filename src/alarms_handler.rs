// /src/alarms_handler.rs
use defmt::debug;
use crate::constants::{ALARMS_CHANNELS_AMOUNT, ALARMS_MESSAGE_STRING_LENGTH, ALARMS_STACK_DEPTH};

const FIRST_STACK_INDEX: usize = 0;
const SECOND_STACK_INDEX: usize = 1;

/// Core interface for alarm tracking functionality
pub trait AlarmTracker {
    fn push(&mut self, alarms: &[bool; ALARMS_CHANNELS_AMOUNT]);
    fn has_changes(&self) -> bool;
    fn export_bits(&mut self) -> [char; ALARMS_MESSAGE_STRING_LENGTH];
    fn import_bits(&mut self, bits: [char; ALARMS_MESSAGE_STRING_LENGTH]);
}

pub struct AlarmStack {
    stack: [[bool; ALARMS_CHANNELS_AMOUNT]; ALARMS_STACK_DEPTH],
    counter: usize,
}

impl AlarmStack {
    // Made this a const function for static initialization
    pub const fn new() -> Self {
        Self {
            stack: [[false; ALARMS_CHANNELS_AMOUNT]; ALARMS_STACK_DEPTH],
            counter: 0,
        }
    }

    pub fn get_stack_view(&self) -> [[bool; ALARMS_CHANNELS_AMOUNT]; ALARMS_STACK_DEPTH] {
        self.stack
    }
}

impl AlarmTracker for AlarmStack {
    fn push(&mut self, alarms: &[bool; ALARMS_CHANNELS_AMOUNT]) {
        if self.counter < ALARMS_STACK_DEPTH {
            self.stack[self.counter] = *alarms;
            self.counter += 1;
        } else {
            let first_row = self.stack[FIRST_STACK_INDEX];
            for idx in 0..ALARMS_CHANNELS_AMOUNT {
                if first_row[idx] != self.stack[SECOND_STACK_INDEX][idx] {
                    continue;
                }
                self.stack[SECOND_STACK_INDEX][idx] =
                    self.stack[ALARMS_STACK_DEPTH - 1][idx];
            }
            self.stack[ALARMS_STACK_DEPTH - 1] = *alarms;
        }
    }
    
    fn has_changes(&self) -> bool {
        for col in 0..ALARMS_CHANNELS_AMOUNT {
            let first = self.stack[0][col];
            for row in 1..ALARMS_STACK_DEPTH {
                if self.stack[row][col] != first {
                    return true;
                }
            }
        }
        false
    }
    
    fn export_bits(&mut self) -> [char; ALARMS_MESSAGE_STRING_LENGTH] {
        debug!("Exporting alarm bits...");
        
        let mut result = ['0'; ALARMS_MESSAGE_STRING_LENGTH];
        for col in 0..ALARMS_CHANNELS_AMOUNT {
            let mut acc: u8 = 0;
            for row in 0..ALARMS_STACK_DEPTH {
                if self.stack[row][col] {
                    acc |= 1 << row;
                }
            }
            result[col] = (acc + b'0') as char;
        }

        self.stack[FIRST_STACK_INDEX] = self.stack[ALARMS_STACK_DEPTH - 1];
        self.counter = 1;
        result
    }
    
    fn import_bits(&mut self, bits: [char; ALARMS_MESSAGE_STRING_LENGTH]) {
        for col in 0..ALARMS_CHANNELS_AMOUNT {
            let digit = (bits[col] as u8).saturating_sub(b'0');
            for row in 0..ALARMS_STACK_DEPTH {
                self.stack[row][col] = ((digit >> row) & 1) != 0;
            }
        }
    }
}

// Added Default implementation back
impl Default for AlarmStack {
    fn default() -> Self {
        Self::new()
    }
}