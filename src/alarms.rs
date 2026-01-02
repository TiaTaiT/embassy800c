use crate::constants::{ALARMS_CHANNELS_AMOUNT, ALARMS_STACK_DEPTH, ALARMS_MESSAGE_STRING_LENGTH};

#[derive(Clone, Copy)]
pub struct AlarmStack {
    stack: [[bool; ALARMS_CHANNELS_AMOUNT]; ALARMS_STACK_DEPTH],
    counter: usize,
}

impl AlarmStack {
    pub fn new() -> Self {
        Self {
            stack: [[false; ALARMS_CHANNELS_AMOUNT]; ALARMS_STACK_DEPTH],
            counter: 0,
        }
    }

    pub fn push(&mut self, alarms: &[bool; ALARMS_CHANNELS_AMOUNT]) {
        if self.counter < ALARMS_STACK_DEPTH {
            self.stack[self.counter] = *alarms;
            self.counter += 1;
        } else {
            for i in 0..ALARMS_STACK_DEPTH - 1 {
                self.stack[i] = self.stack[i+1];
            }
            self.stack[ALARMS_STACK_DEPTH - 1] = *alarms;
        }
    }

    pub fn has_changes(&self) -> bool {
        let first = self.stack[0];
        for row in 1..ALARMS_STACK_DEPTH {
            if self.stack[row] != first {
                return true;
            }
        }
        false
    }

    pub fn export_bits(&mut self) -> [char; ALARMS_MESSAGE_STRING_LENGTH] {
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

        // Reset history but keep latest state as baseline
        self.stack[0] = self.stack[ALARMS_STACK_DEPTH - 1];
        self.counter = 1;
        result
    }

    pub fn import_bits(&mut self, bits: [char; ALARMS_MESSAGE_STRING_LENGTH]) {
        for col in 0..ALARMS_CHANNELS_AMOUNT {
            let digit = (bits[col] as u8).saturating_sub(b'0');
            for row in 0..ALARMS_STACK_DEPTH {
                self.stack[row][col] = ((digit >> row) & 1) != 0;
            }
        }
    }
}