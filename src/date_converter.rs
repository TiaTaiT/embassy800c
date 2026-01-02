// /src/date_converter.rs
use core::fmt::{self, Write};

use crate::rtc::GsmTime;

// Fixed buffer to format "yymmddhhmmss" (12 characters)
pub struct TimeBuffer {
    buf: [u8; 12],
    pos: usize,
}

impl TimeBuffer {
    pub fn new() -> Self {
        Self {
            buf: [0; 12],
            pos: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.pos]).unwrap()
    }
}

impl Write for TimeBuffer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        if self.pos + bytes.len() > self.buf.len() {
            return Err(fmt::Error);
        }
        self.buf[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }
}

pub fn format_gsm_time(time: &GsmTime) -> TimeBuffer {
    let mut buf = TimeBuffer::new();
    write!(&mut buf, "{:02}{:02}{:02}{:02}{:02}{:02}",
        time.year,
        time.month,
        time.day,
        time.hour,
        time.minute,
        time.second
    ).unwrap();
    buf
}