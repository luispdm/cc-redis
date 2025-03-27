mod cmd;
mod deserializer;
mod resp;

use cmd::{request::Request, response::Response};
use deserializer::Deserializer;

use log::{error, trace};

use bytes::BytesMut;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();
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
                        trace!(
                            "received: {:?}",
                            String::from_utf8(buf[cursor..cursor + n].to_vec())
                        );

                        let reply = Deserializer::default()
                            .deserialize_msg(&buf[cursor..cursor + n])
                            .map_or_else(
                                |e| Response::SimpleError(e.to_string()),
                                |des| {
                                    trace!("deserialized {:?}", des);
                                    Request::try_from(des).map_or_else(
                                        |e| Response::SimpleError(e.to_string()),
                                        |cmd| cmd.execute(),
                                    )
                                },
                            );

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
