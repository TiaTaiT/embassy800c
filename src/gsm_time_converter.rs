use heapless::Vec;

use crate::rtc::GsmTime;

impl GsmTime {
    fn parse_u8(s: &[u8]) -> Option<u8> {
        let mut result = 0u8;
        for &byte in s {
            if byte < b'0' || byte > b'9' {
                return None;
            }
            let digit = byte - b'0';
            result = result.checked_mul(10)?.checked_add(digit)?;
        }
        Some(result)
    }

    fn parse_u16_to_u8_year(s: &[u8]) -> Option<u8> {
        let mut result = 0u16;
        for &byte in s {
            if byte < b'0' || byte > b'9' {
                return None;
            }
            let digit = (byte - b'0') as u16;
            result = result.checked_mul(10)?.checked_add(digit)?;
        }
        Some((result % 100) as u8)
    }

    pub fn parse_gsm_time(&self, date: &str) -> Option<GsmTime> {
        // Create a buffer for the result
        let mut result_buf = [0u8; 32];
        let mut result_len = 0;

        // Iterate through date and build normalized result
        for byte in date.bytes() {
            if result_len >= result_buf.len() - 1 {
                break;
            }
            
            if byte >= b'0' && byte <= b'9' {
                // Copy digit
                result_buf[result_len] = byte;
            } else {
                // Insert comma for non-digit
                result_buf[result_len] = b',';
            }
            result_len += 1;
        }

        // Split by commas into heapless Vec
        let mut parts: Vec<&[u8], 8> = Vec::new();
        let mut start = 0;

        for (i, &byte) in result_buf[..result_len].iter().enumerate() {
            if byte == b',' {
                if start < i {
                    let _ = parts.push(&result_buf[start..i]);
                }
                start = i + 1;
            }
        }

        // Add final part if exists
        if start < result_len {
            let _ = parts.push(&result_buf[start..result_len]);
        }

        // Filter out empty parts
        let valid_parts: Vec<&[u8], 8> = parts.into_iter()
            .filter(|part| !part.is_empty())
            .collect();

        // Need exactly 6 parts for GSM time
        if valid_parts.len() != 6 {
            return None;
        }

        let year = if valid_parts[0].len() > 2 {
            Self::parse_u16_to_u8_year(valid_parts[0])?
        } else {
            Self::parse_u8(valid_parts[0])?
        };
        
        let month = Self::parse_u8(valid_parts[1])?;
        let day = Self::parse_u8(valid_parts[2])?;
        let hour = Self::parse_u8(valid_parts[3])?;
        let minute = Self::parse_u8(valid_parts[4])?;
        let second = Self::parse_u8(valid_parts[5])?;

        // Validate ranges
        if month < 1 || month > 12 || day < 1 || day > 31 || 
           hour > 23 || minute > 59 || second > 59 {
            return None;
        }

        Some(GsmTime { year, month, day, hour, minute, second })
    }
}