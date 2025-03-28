mod cmd;
mod db;
mod deserializer;
mod resp;

use std::{collections::HashMap, sync::Arc};

use cmd::{request::Request, response::Response};
use db::Db;
use deserializer::Deserializer;

use log::{error, trace};

use bytes::BytesMut;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::RwLock,
};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();
    let db = Arc::new(RwLock::new(HashMap::<String, String>::new()));

    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    loop {
        let (mut stream, _) = listener.accept().await?;
        let db = Arc::clone(&db);

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
                        trace!(
                            "received: {:?}",
                            String::from_utf8(buf[cursor..cursor + n].to_vec())
                        );

                        let reply =
                            deserialize_and_execute(&buf[cursor..cursor + n], db.clone()).await; // TODO how to avoid re-cloning?

                        writer.write_all(&reply.serialize()).await.unwrap();
                        writer.flush().await.unwrap();
                        cursor += n;
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

async fn deserialize_and_execute(msg: &[u8], db: Db) -> Response {
    let maybe_des = Deserializer::default()
        .deserialize_msg(msg)
        .map_err(|e| Response::SimpleError(e.to_string()));
    if let Err(e) = maybe_des {
        return e;
    }

    let des = maybe_des.unwrap();
    trace!("deserialized {:?}", des);
    match Request::try_from(des) {
        Err(e) => Response::SimpleError(e.to_string()),
        Ok(cmd) => cmd.execute(db).await,
    }
}
