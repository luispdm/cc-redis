use std::{num::ParseIntError, str};

use bytes::BytesMut;
use thiserror::Error;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const ARRAY_TYPE: u8 = b'*';
const BULK_STRING_TYPE: u8 = b'$';
const ERROR_TYPE: u8 = b'-';
const INTEGER_TYPE: u8 = b':';
const NULL_TYPE: u8 = b'_';
const SIMPLE_STRING_TYPE: u8 = b'+';
const CR: u8 = b'\r';
const LF: u8 = b'\n';

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    loop {
        let (mut stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let (mut reader, mut writer) = stream.split();
            let mut buf = BytesMut::with_capacity(4096);
            let mut cursor = 0;
            loop {
                match reader.read_buf(&mut buf).await {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        println!(
                            "Received {:?}",
                            String::from_utf8(buf[cursor..cursor + n].to_vec())
                        );
                        let reply = {
                            Deserializer::default()
                                .deserialize_msg(&buf[cursor..cursor + n])
                                .map_err(|e| ReplyCmd::SimpleError(e.to_string()))
                                .map(|des| {
                                    println!("Deserialized {:?}", des);
                                    match RequestCmd::try_from(des) {
                                        Ok(cmd) => cmd.execute(),
                                        Err(e) => ReplyCmd::SimpleError(e.to_string()),
                                    }
                                })
                                .unwrap()
                        };

                        writer.write_all(&reply.serialize()).await.unwrap();
                        writer.flush().await.unwrap();
                        cursor += n;
                    }
                    Err(e) => {
                        println!("Failed to read from socket: {:?}", e);
                        break;
                    }
                }
            }
        });
    }
}

enum RequestCmd {
    Ping(Option<String>),
}

impl RequestCmd {
    fn execute(self) -> ReplyCmd {
        match self {
            Self::Ping(val) => {
                if let Some(msg) = val {
                    ReplyCmd::BulkString(msg.to_string())
                } else {
                    ReplyCmd::SimpleString("PONG".to_string())
                }
            }
        }
    }
}

impl TryFrom<Vec<String>> for RequestCmd {
    type Error = ClientError;

    fn try_from(params: Vec<String>) -> Result<Self, Self::Error> {
        if params.is_empty() {
            return Err(ClientError::UnknownCommand("".to_string()));
        }

        match params[0].to_lowercase().as_str() {
            "ping" => {
                if params.len() > 2 {
                    Err(ClientError::WrongNumberOfArguments("ping".to_string()))
                } else {
                    Ok(RequestCmd::Ping(params.get(1).cloned()))
                }
            }
            c => Err(ClientError::UnknownCommand(c.to_string())),
        }
    }
}

#[derive(Debug, Error)]
enum ClientError {
    #[error("unknown command '{0}'")]
    UnknownCommand(String),
    #[error("wrong number of arguments for '{0}' command")]
    WrongNumberOfArguments(String),
}

#[derive(Debug)]
enum ReplyCmd {
    Null,
    SimpleString(String),
    BulkString(String),
    Integer(String),
    SimpleError(String),
}

impl ReplyCmd {
    fn serialize(&self) -> Vec<u8> {
        let mut bytes = vec![];
        match self {
            ReplyCmd::Null => {
                bytes.push(NULL_TYPE);
                bytes.push(CR);
                bytes.push(LF);
            }
            ReplyCmd::SimpleString(s) => {
                bytes.push(SIMPLE_STRING_TYPE);
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(CR);
                bytes.push(LF);
            }
            ReplyCmd::Integer(s) => {
                bytes.push(INTEGER_TYPE);
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(CR);
                bytes.push(LF);
            }
            ReplyCmd::SimpleError(s) => {
                bytes.push(ERROR_TYPE);
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(CR);
                bytes.push(LF);
            }
            ReplyCmd::BulkString(s) => {
                bytes.push(BULK_STRING_TYPE);
                bytes.extend_from_slice(s.len().to_string().as_bytes());
                bytes.push(CR);
                bytes.push(LF);
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(CR);
                bytes.push(LF);
            }
        }
        bytes
    }
}

#[derive(Debug, Error)]
#[error("\r\n not found")]
struct CrLfNotFound;

#[derive(Default)]
struct Deserializer {
    cursor: usize,
    cr_pos: usize,
    lf_pos: usize,
}

#[derive(Debug, Error)]
enum DeserializeError {
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
    fn deserialize_msg(&mut self, msg: &[u8]) -> Result<Vec<String>, DeserializeError> {
        if msg.get(self.cursor).is_none_or(|c| *c != ARRAY_TYPE) {
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
        if msg[self.cursor] != BULK_STRING_TYPE {
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
    fn serialize_null() {
        let reply = ReplyCmd::Null;
        assert_eq!(reply.serialize(), b"_\r\n");
    }

    #[test]
    fn serialize_simple_string() {
        let reply = ReplyCmd::SimpleString("".to_string());
        assert_eq!(reply.serialize(), b"+\r\n");

        let reply = ReplyCmd::SimpleString("OK".to_string());
        assert_eq!(reply.serialize(), b"+OK\r\n");

        let reply = ReplyCmd::SimpleString("Hello World".to_string());
        assert_eq!(reply.serialize(), b"+Hello World\r\n");

        let reply = ReplyCmd::SimpleString("こんにちは".to_string());
        assert_eq!(reply.serialize(), "+こんにちは\r\n".as_bytes());
    }

    #[test]
    fn serialize_integer() {
        let reply = ReplyCmd::Integer("0".to_string());
        assert_eq!(reply.serialize(), b":0\r\n");

        let reply = ReplyCmd::Integer("42".to_string());
        assert_eq!(reply.serialize(), b":42\r\n");

        let reply = ReplyCmd::Integer("-1".to_string());
        assert_eq!(reply.serialize(), b":-1\r\n");
    }

    #[test]
    fn serialize_simple_error() {
        let reply = ReplyCmd::SimpleError("Error".to_string());
        assert_eq!(reply.serialize(), b"-Error\r\n");

        let reply = ReplyCmd::SimpleError("ERR unknown command".to_string());
        assert_eq!(reply.serialize(), b"-ERR unknown command\r\n");
    }

    #[test]
    fn serialize_bulk_string() {
        let reply = ReplyCmd::BulkString("".to_string());
        assert_eq!(reply.serialize(), b"$0\r\n\r\n");

        let reply = ReplyCmd::BulkString("hello world".to_string());
        assert_eq!(reply.serialize(), b"$11\r\nhello world\r\n");

        let reply = ReplyCmd::BulkString("💸".to_string());
        assert_eq!(reply.serialize(), b"$4\r\n\xF0\x9F\x92\xB8\r\n");
    }

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
