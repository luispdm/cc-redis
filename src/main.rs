use std::str;

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
    #[error("invalid array size")]
    InvalidArraySize,
    #[error("bulk string expected")]
    BulkStringExpected,
    #[error("malformed bulk string")]
    MalformedBulkString,
}

fn main() {
    // println!("test... {:?}", deserialize_msg(b"*1\r\n$4\r\nabcd\r\n"));
    println!(
        "test2.. {:?}",
        deserialize_msg(b"*2\r\n$4\r\necho\r\n$11\r\nhello world\r\n")
    );
    // print!("$ represents a... ");
    // let _ = deserialize_msg(b"$");
    // print!("* represents a... ");
    // let _ = deserialize_msg(b"*");
    // print!("+ represents a... ");
    // println!("A represents a... {:?}", deserialize_msg(b"A"));
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

fn deserialize_msg(msg: &[u8]) -> Result<(), ClientError> {
    println!("msg is: {:?}", msg);
    println!("msg.len() is: {}", msg.len());
    let mut cursor = 0;
    if msg.get(cursor).is_none_or(|c| *c != ARRAY_TYPE) {
        return Err(ClientError::InvalidStartOfMsg);
    }
    // advance to the first CRLF to find out how many elements the array has
    cursor += 1;
    let (mut cr_pos, mut lf_pos) =
        get_cr_lf_positions(msg, cursor).ok_or(ClientError::MalformedArray)?;
    println!("cr_pos: {}, lf_pos: {}", cr_pos, lf_pos);
    let array_size = get_number_from_string(&msg[cursor..cr_pos])?;
    println!("iterations to run: {}", array_size);
    let mut iterations = 0u32;
    // extract the bulk strings
    while iterations < array_size {
        cursor = lf_pos + 1;
        if msg.get(cursor).is_none() {
            return Err(ClientError::MalformedArray);
        }
        if msg[cursor] != BULK_STRING_TYPE {
            return Err(ClientError::BulkStringExpected);
        }
        // get the bulk string size
        cursor += 1;
        (cr_pos, lf_pos) = get_cr_lf_positions(msg, cursor).ok_or(ClientError::MalformedBulkString)?;
        println!("cursor: {}, cr_pos: {}, lf_pos: {}", cursor, cr_pos, lf_pos);
        let bulk_string_size = get_number_from_string(&msg[cursor..cr_pos])?;
        println!("bulk_string_size: {}", bulk_string_size);
        // extract the bulk string data (make sure it's consistent with the size)
        cursor = lf_pos + 1;
        if msg.get(cursor).is_none() || msg[cursor..].len() < bulk_string_size as usize {
            return Err(ClientError::MalformedBulkString);
        }
        let bulk_string = &msg[cursor..cursor + bulk_string_size as usize];
        println!("cursor: {}", cursor);
        println!(
            "bulk_string: {:?}",
            str::from_utf8(bulk_string).map_err(|_| ClientError::MalformedBulkString)?
        );
        // advance to the next CRLF
        cursor += bulk_string_size as usize;
        println!("cursor: {}", cursor);
        // check for CRLF after the bulk string
        if msg.get(cursor).is_none_or(|c| *c != CR) {
            return Err(ClientError::MalformedBulkString);
        }
        cursor += 1;
        if msg.get(cursor).is_none_or(|c| *c != LF) {
            return Err(ClientError::MalformedBulkString);
        }
        lf_pos = cursor;
        iterations += 1;
        println!("cursor: {}", cursor);
    }
    // TODO create a test that returns this error
    if iterations != array_size {
        return Err(ClientError::InvalidArraySize);
    }
    // make sure there's nothing else after the last CRLF
    cursor += 1;
    println!("cursor: {}", cursor);
    if msg.get(cursor).is_some() {
        return Err(ClientError::MalformedArray);
    }
    Ok(())
}

fn get_number_from_string(s: &[u8]) -> Result<u32, ClientError> {
    str::from_utf8(s)
        .unwrap_or_default()
        .parse::<u32>()
        .map_err(|_| ClientError::MalformedArray)
}

fn get_cr_lf_positions(msg: &[u8], cursor: usize) -> Option<(usize, usize)> {
    let mut cursor = cursor;
    while cursor < msg.len() - 1 {
        if msg[cursor] == CR && msg[cursor + 1] == LF {
            return Some((cursor, cursor + 1));
        }
        cursor += 1;
    }
    None
}
