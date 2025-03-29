mod cmd;
mod db;
mod deserializer;
mod resp;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use cmd::{request::Request, response::Response};
use db::Db;
use deserializer::Deserializer;

use log::{error, trace, warn};

use bytes::BytesMut;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();
    let db = Arc::new(Mutex::new(HashMap::<String, String>::new()));

    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    loop {
        let (mut stream, _) = listener.accept().await?;
        let db = Arc::clone(&db);

        tokio::spawn(async move {
            // TODO evaluate `BufReader` and `BufWriter` over `ReadHalf` and `WriteHalf`
            let (mut reader, mut writer) = stream.split();
            let mut buf = BytesMut::with_capacity(1024);
            loop {
                match reader.read_buf(&mut buf).await {
                    Ok(0) => {
                        break;
                    }
                    Ok(_) => {
                        trace!("received: {:?}", String::from_utf8(buf[..].to_vec()));

                        let reply = deserialize_and_execute(&buf[..], &db);

                        if let Err(e) = writer.write_all(&reply.serialize()).await {
                            error!("failed to write to socket: {}", e)
                        }
                        if let Err(e) = writer.flush().await {
                            error!("failed to flush to socket: {}", e)
                        }
                        buf.clear();
                    }
                    Err(e) => {
                        error!("failed to read from socket: {}", e);
                        break;
                    }
                }
            }
        });
    }
}

fn deserialize_and_execute(msg: &[u8], db: &Db) -> Response {
    let maybe_des = Deserializer::default()
        .deserialize_msg(msg)
        .map_err(|e| Response::SimpleError(e.to_string()));
    if let Err(e) = maybe_des {
        warn!("deserialization failed: {:?}", e);
        return e;
    }

    let des = maybe_des.unwrap();
    trace!("deserialized {:?}", des);
    match Request::try_from(des) {
        Err(e) => Response::SimpleError(e.to_string()),
        Ok(cmd) => cmd.execute(db),
    }
}
