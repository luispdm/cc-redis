use std::{num::ParseIntError, str};

use bytes::BytesMut;
use thiserror::Error;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const ARRAY_TYPE: u8 = b'*';
const BULK_STRING_TYPE: u8 = b'$';
const CR: u8 = b'\r';
const LF: u8 = b'\n';

#[derive(Debug, Error)]
enum ClientError {
    #[error("message must be an array")]
    InvalidStartOfMsg,
    #[error("invalid array")]
    MalformedArray,
    #[error("bulk string expected")]
    BulkStringExpected,
    #[error("malformed bulk string")]
    MalformedBulkString,
}

fn main() {
    Deserializer::default().deserialize_msg(b"*1\r\n$4\r\nping\r\n");
    Deserializer::default().deserialize_msg(b"*2\r\n$4\r\necho\r\n$11\r\nhello world\r\n");
    Deserializer::default().deserialize_msg("*2\r\n$4\r\necho\r\n$4\r\n💸\r\n".as_bytes());
    Deserializer::default().deserialize_msg(b"*2\r\n$3\r\nget\r\n$3\r\nkey\r\n");
}

// #[tokio::main]
// async fn main() -> io::Result<()> {
//     let listener = TcpListener::bind("127.0.0.1:6379").await?;
//     loop {
//         let (mut stream, _) = listener.accept().await?;
//         tokio::spawn(async move {
//             let (mut reader, mut writer) = stream.split();
//             let mut buf = BytesMut::with_capacity(4096);
//             // loop {
//                 match reader.read_buf(&mut buf).await {
//                     Ok(0) => {
//                         println!("0 bytes received");
//                         // break;
//                     }
//                     Ok(n) => {
//                         println!("Received {:?}", String::from_utf8(buf[..n].to_vec()));
//                         writer.write_all(&buf).await.unwrap();
//                         writer.flush().await.unwrap();
//                     }
//                     Err(e) => {
//                         println!("Failed to read from socket: {:?}", e);
//                         // break;
//                     }
//                 }
//             // }
//         });
//     }
// }

#[derive(Debug, Error)]
#[error("\r\n not found")]
struct CrLfNotFound;

#[derive(Default)]
struct Deserializer {
    cursor: usize,
    cr_pos: usize,
    lf_pos: usize,
}

impl Deserializer {
    fn deserialize_msg(&mut self, msg: &[u8]) -> Result<(), ClientError> {
        println!("msg is: {:?}", msg);
        println!("msg.len() is: {}", msg.len());
        if msg.get(self.cursor).is_none_or(|c| *c != ARRAY_TYPE) {
            return Err(ClientError::InvalidStartOfMsg);
        }
        // advance to the first CRLF to find out how many elements the array has
        self.cursor += 1;
        self.update_cr_lf(msg).map_err(|_| ClientError::MalformedArray)?;
        println!("cr_pos: {}, lf_pos: {}", self.cr_pos, self.lf_pos);
        let array_size = get_u32_from_string(&msg[self.cursor..self.cr_pos]).map_err(|_| ClientError::MalformedArray)?;
        println!("iterations to run: {}", array_size);
        // extract the bulk strings
        for _ in 0..array_size {
            self.cursor = self.lf_pos + 1;
            if msg.get(self.cursor).is_none() {
                return Err(ClientError::MalformedArray); // TODO MalformedBulkString? Debatable
            }
            if msg[self.cursor] != BULK_STRING_TYPE {
                return Err(ClientError::BulkStringExpected);
            }
            // get the bulk string size
            self.cursor += 1;
            self.update_cr_lf(msg).map_err(|_| ClientError::MalformedBulkString)?;
            println!("cursor: {}, cr_pos: {}, lf_pos: {}", self.cursor, self.cr_pos, self.lf_pos);
            let bulk_string_size = get_u32_from_string(&msg[self.cursor..self.cr_pos]).map_err(|_| ClientError::MalformedBulkString)?;
            println!("bulk_string_size: {}", bulk_string_size);
            // extract the bulk string data (make sure it's consistent with the size)
            self.cursor = self.lf_pos + 1;
            if msg.get(self.cursor).is_none() || msg[self.cursor..].len() < bulk_string_size as usize {
                return Err(ClientError::MalformedBulkString);
            }
            let bulk_string = &msg[self.cursor..self.cursor + bulk_string_size as usize];
            println!("cursor: {}", self.cursor);
            println!(
                "bulk_string: {:?}",
                str::from_utf8(bulk_string).map_err(|_| ClientError::MalformedBulkString)?
            );
            // advance to the next CRLF
            self.cursor += bulk_string_size as usize;
            println!("cursor: {}", self.cursor);
            if msg.get(self.cursor).is_none_or(|c| *c != CR) {
                return Err(ClientError::MalformedBulkString);
            }
            self.cursor += 1;
            if msg.get(self.cursor).is_none_or(|c| *c != LF) {
                return Err(ClientError::MalformedBulkString);
            }
            self.lf_pos = self.cursor;
            println!("cursor: {}", self.cursor);
        }
        // make sure there's nothing else after the last CRLF
        self.cursor += 1;
        println!("cursor: {}", self.cursor);
        if msg.get(self.cursor).is_some() {
            return Err(ClientError::MalformedArray);
        }
        Ok(())
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
    str::from_utf8(s)
        .unwrap_or_default()
        .parse::<u32>()
}
