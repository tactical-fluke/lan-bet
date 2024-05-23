use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const BUFFER_LIMIT: usize = 1048576;

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum Request {
    Login { user: String },
    WhoAmI,
    WagerData,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Response {
    Ok(RequestResponse),
    Error,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum RequestResponse {
    None,
    WhoAmI(String),
    WagerData(Vec<crate::database::WagerInfo>),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Packet {
    RequestPacket(Request),
    ResponsePacket(Response),
}

pub struct Connection {
    connection: TcpStream,
    buf: Vec<u8>,
    cursor: usize,
}

impl Connection {
    pub fn from_tcp_stream(connection: TcpStream) -> Self {
        Self {
            connection,
            buf: vec![0; 4096],
            cursor: 0,
        }
    }

    pub async fn read(&mut self) -> tokio::io::Result<Packet> {
        let mut last_error = tokio::io::Error::new(ErrorKind::Other, "");
        let mut parse_cursor = self.cursor;
        loop {
            while parse_cursor <= self.cursor {
                let res = rmp_serde::from_slice(&self.buf[..parse_cursor]);
                match res {
                    Ok(req) => {
                        self.buf.drain(..parse_cursor);
                        self.cursor = parse_cursor - self.cursor;
                        return tokio::io::Result::Ok(req);
                    }
                    Err(_) => {
                        last_error =
                            tokio::io::Error::new(ErrorKind::InvalidData, "malformed request")
                    }
                }
                parse_cursor += 1;
            }

            self.grow_buffer_if_needed();
            if self.buf.len() >= BUFFER_LIMIT {
                return Err(last_error);
            }
            let n = self.connection.read(&mut self.buf[self.cursor..]).await?;
            if n == 0 {
                let e = tokio::io::Error::new(ErrorKind::ConnectionAborted, "connection closed");
                return Err(e);
            }
            self.cursor += n;
        }
    }

    pub async fn send(&mut self, data: Packet) -> tokio::io::Result<()> {
        self.connection
            .write_all(&rmp_serde::to_vec(&data).unwrap())
            .await
    }

    fn grow_buffer_if_needed(&mut self) {
        if self.buf.len() == self.cursor {
            self.buf.resize(self.buf.len() * 2, 0);
        }
    }
}
