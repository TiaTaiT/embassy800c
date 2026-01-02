// /src/phone_book.rs
use heapless::String;
use defmt::info;

use crate::constants::MAX_PHONE_LENGTH;

const MAX_PHONE_COUNT: usize = 8;

pub struct PhoneBook {
    phones: [Option<String<MAX_PHONE_LENGTH>>; MAX_PHONE_COUNT],
    count: usize,
}

impl PhoneBook {
    pub const fn new() -> Self {
        Self {
            phones: [None, None, None, None, None, None, None, None],
            count: 0,
        }
    }

    pub fn add_number(&mut self, number: &str) -> Result<(), &'static str> {
        info!("Trying to add number {}", number);
        if self.count >= MAX_PHONE_COUNT {
            return Err("Phone book full");
        }
        if number.len() >= MAX_PHONE_LENGTH {
            return Err("Phone number too long");
        }
        if self.contains(number) {
            return Err("Phone number already exists");
        }

        let mut s: String<MAX_PHONE_LENGTH> = String::new();
        if s.push_str(number).is_err() {
            return Err("Failed to add number");
        }

        self.phones[self.count] = Some(s);
        self.count += 1;
        Ok(())
    }

    pub fn get_first(&self) -> Option<&str> {
        self.phones.get(0).and_then(|opt| opt.as_deref())
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        if index < self.count {
            self.phones[index].as_deref()
        } else {
            None
        }
    }

    pub fn contains(&self, number: &str) -> bool {
        self.phones.iter().flatten().any(|entry| entry.as_str() == number)
    }
}