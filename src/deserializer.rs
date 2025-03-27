use std::{num::ParseIntError, str};
use thiserror::Error;

use crate::resp::types::{ARRAY, BULK_STRING, CR, LF};

#[derive(Debug, Error)]
#[error("\r\n not found")]
struct CrLfNotFound;

#[derive(Default)]
pub struct Deserializer {
    cursor: usize,
    cr_pos: usize,
    lf_pos: usize,
}

#[derive(Debug, Error)]
pub enum DeserializeError {
    #[error("message must be an array")]
    InvalidStartOfMsg,
    #[error("invalid array")]
    MalformedArray,
    #[error("bulk string expected")]
    BulkStringExpected,
    #[error("malformed bulk string")]
    MalformedBulkString,
}

impl Deserializer {
    pub fn deserialize_msg(&mut self, msg: &[u8]) -> Result<Vec<String>, DeserializeError> {
        if msg.get(self.cursor).is_none_or(|c| *c != ARRAY) {
            return Err(DeserializeError::InvalidStartOfMsg);
        }

        // advance to the first CRLF to find out how many elements the array has
        self.cursor += 1;
        self.update_cr_lf(msg)
            .map_err(|_| DeserializeError::MalformedArray)?;
        let array_size = get_u32_from_string(&msg[self.cursor..self.cr_pos])
            .map_err(|_| DeserializeError::MalformedArray)?;

        // extract the bulk strings
        let mut params = vec![];
        for _ in 0..array_size {
            self.check_bulk_string_type(msg)?;

            let (bulk_string, bulk_string_size) = self.extract_bulk_string(msg)?;
            params.push(bulk_string);

            self.jump_to_lf(msg, bulk_string_size as usize)?;
        }

        // make sure there's nothing else after the last CRLF
        self.cursor += 1;
        if msg.get(self.cursor).is_some() {
            return Err(DeserializeError::MalformedArray);
        }

        Ok(params)
    }

    fn check_bulk_string_type(&mut self, msg: &[u8]) -> Result<(), DeserializeError> {
        self.cursor = self.lf_pos + 1;
        if msg.get(self.cursor).is_none() {
            return Err(DeserializeError::MalformedBulkString);
        }
        if msg[self.cursor] != BULK_STRING {
            return Err(DeserializeError::BulkStringExpected);
        }
        Ok(())
    }

    fn jump_to_lf(&mut self, msg: &[u8], bulk_string_size: usize) -> Result<(), DeserializeError> {
        self.cursor += bulk_string_size;
        if msg.get(self.cursor).is_none_or(|c| *c != CR) {
            return Err(DeserializeError::MalformedBulkString);
        }
        self.cursor += 1;
        if msg.get(self.cursor).is_none_or(|c| *c != LF) {
            return Err(DeserializeError::MalformedBulkString);
        }
        self.lf_pos = self.cursor;
        Ok(())
    }

    fn extract_bulk_string(&mut self, msg: &[u8]) -> Result<(String, u32), DeserializeError> {
        // get the size
        self.cursor += 1;
        self.update_cr_lf(msg)
            .map_err(|_| DeserializeError::MalformedBulkString)?;

        let bulk_string_size = get_u32_from_string(&msg[self.cursor..self.cr_pos])
            .map_err(|_| DeserializeError::MalformedBulkString)?;

        // get the data (make sure it's consistent with the size)
        self.cursor = self.lf_pos + 1;
        if msg.get(self.cursor).is_none() || msg[self.cursor..].len() < bulk_string_size as usize {
            return Err(DeserializeError::MalformedBulkString);
        }
        let bulk_string_bytes = &msg[self.cursor..self.cursor + bulk_string_size as usize];
        let bulk_string = str::from_utf8(bulk_string_bytes)
            .map(|s| s.to_owned())
            .map_err(|_| DeserializeError::MalformedBulkString)?;

        Ok((bulk_string, bulk_string_size))
    }

    fn update_cr_lf(&mut self, msg: &[u8]) -> Result<(), CrLfNotFound> {
        let mut cursor = self.cursor;
        while cursor < msg.len() - 1 {
            if msg[cursor] == CR && msg[cursor + 1] == LF {
                self.cr_pos = cursor;
                self.lf_pos = cursor + 1;
                return Ok(());
            }
            cursor += 1;
        }
        Err(CrLfNotFound)
    }
}

fn get_u32_from_string(s: &[u8]) -> Result<u32, ParseIntError> {
    str::from_utf8(s).unwrap_or_default().parse::<u32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_ok() {
        let msg = b"*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n";
        let expected_params = vec!["SET", "key", "value"];
        let mut deserializer = Deserializer::default();
        assert_eq!(expected_params, deserializer.deserialize_msg(msg).unwrap());

        let msg = b"*1\r\n$0\r\n\r\n";
        let expected_params = vec![""];
        let mut deserializer = Deserializer::default();
        assert_eq!(expected_params, deserializer.deserialize_msg(msg).unwrap());

        let msg = b"*1\r\n$4\r\n\xF0\x9F\x92\xB8\r\n";
        let expected_params = vec!["💸"];
        let mut deserializer = Deserializer::default();
        assert_eq!(expected_params, deserializer.deserialize_msg(msg).unwrap());
    }

    #[test]
    fn deserialize_invalid_start() {
        let msg = b"$3\r\nGET\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::InvalidStartOfMsg
        ));

        let msg = b"";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::InvalidStartOfMsg
        ));
    }

    #[test]
    fn deserialize_invalid_array_size() {
        let msg = b"*x\r\n$4\r\nPING\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedArray
        ));
    }

    #[test]
    fn deserialize_array_size_bigger() {
        let msg = b"*2\r\n$4\r\nPING\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedBulkString
        ));
    }

    #[test]
    fn deserialize_array_size_smaller() {
        let msg = b"*1\r\n$4\r\nECHO\r\n$5\r\nworld\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedArray
        ));
    }

    #[test]
    fn deserialize_array_size_missing_terminator() {
        let msg = b"*1$4\r\nPING\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedArray
        ));
    }

    #[test]
    fn deserialize_bulk_string_expected() {
        let msg = b"*1\r\n[123\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::BulkStringExpected
        ));
    }

    #[test]
    fn deserialize_invalid_bulk_string_size() {
        let msg = b"*1\r\n$x\r\nPING\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedBulkString
        ));
    }

    #[test]
    fn deserialize_bulk_string_size_bigger() {
        let msg = b"*1\r\n$10\r\nPING\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedBulkString
        ));
    }

    #[test]
    fn deserialize_bulk_string_size_smaller() {
        let msg = b"*1\r\n$1\r\nPING\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedBulkString
        ));
    }

    #[test]
    fn deserialize_bulk_string_missing_terminator() {
        let msg = b"*1\r\n$4\r\nPING";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedBulkString
        ));
    }

    #[test]
    fn deserialize_bulk_string_size_missing_terminator() {
        let msg = b"*1\r\n$4PING\r\n";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedBulkString
        ));
    }

    #[test]
    fn deserialize_trailing_data() {
        let msg = b"*1\r\n$4\r\nPING\r\nEXTRA";
        let mut deserializer = Deserializer::default();
        assert!(matches!(
            deserializer.deserialize_msg(msg).unwrap_err(),
            DeserializeError::MalformedArray
        ));
    }
}
