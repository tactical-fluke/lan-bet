use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use tokio::net::TcpStream;

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum Request {
    Login { user: String }, // None response
    WhoAmI,
    WagerData,
    ResolveWager{ wager_id: String, winning_option_id: String }, //None response
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Response {
    None,
    SuccessfulLogin{username: String, balance: u64},
    WhoAmI(String),
    WagerData(Vec<crate::Wager>),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Packet {
    RequestPacket(Request),
    ResponsePacket(Response),
    Error,
}

pub struct Connection {
    #[cfg(not(target_arch = "wasm32"))]
    connection: not_wasm::TungsteniteWebSocket,
    #[cfg(target_arch = "wasm32")]
    connection: wasm::WasmWebSocket,
}

impl Connection {
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn from_tcp_stream(connection: TcpStream) -> anyhow::Result<Self> {
        let ws = not_wasm::TungsteniteWebSocket::new(connection).await?;

        Ok(Self {
            connection: ws,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn connect(address: &str) -> anyhow::Result<Self> {
        Ok(Self {
            connection: wasm::WasmWebSocket::new(address)?
        })
    }

    pub async fn read(&mut self) -> anyhow::Result<Packet> {
        Ok(rmp_serde::from_slice(&self.connection.read().await?)?)
    }

    pub async fn send(&mut self, data: Packet) -> anyhow::Result<()> {
        self.connection
            .write_all(&rmp_serde::to_vec(&data).unwrap())
            .await
    }
}

trait WebSocketConnection {
    async fn read<'a>(&'a mut self) -> anyhow::Result<Vec<u8>>;

    async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> anyhow::Result<()>;
}

#[cfg(not(target_arch = "wasm32"))]
mod not_wasm{
    use anyhow::bail;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::Message;
    use crate::network::WebSocketConnection;

    pub struct TungsteniteWebSocket {
        socket: tokio_tungstenite::WebSocketStream<TcpStream>,
    }
    
    impl TungsteniteWebSocket {
        pub async fn new(stream: TcpStream) -> anyhow::Result<Self> {
            let ws_stream = tokio_tungstenite::accept_async(stream).await?;
            Ok(Self { socket: ws_stream })
        }
    }
    
    impl WebSocketConnection for TungsteniteWebSocket {
        async fn read<'a>(&'a mut self) -> anyhow::Result<Vec<u8>> {
            let message = self.socket.next().await.ok_or(anyhow::anyhow!("some error"))??;
            match message {
                Message::Binary(data) => {
                    Ok(data)
                }
                _ => {
                    bail!("incorrect data type received")
                }
            }
        }
    
        async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> anyhow::Result<()> {
            Ok(self.socket.send(Message::Binary(buf.to_vec())).await?)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use gloo::net::websocket::futures::WebSocket;
    use crate::network::WebSocketConnection;
    use futures::{SinkExt, StreamExt};
    use gloo::net::websocket::Message;

    pub struct WasmWebSocket {
        socket: WebSocket,
    }

    impl WasmWebSocket {
        pub fn new(address: &str) -> anyhow::Result<Self> {
            let socket = WebSocket::open(address).unwrap();
            Ok(Self {
                socket
            })
        }
    }
    
    impl WebSocketConnection for WasmWebSocket {
        async fn read<'a>(&'a mut self) -> anyhow::Result<Vec<u8>> {
            let message = self.socket.next().await.unwrap()?;
            if let Message::Bytes(data) = message {
                Ok(data)
            } else {
                Ok(vec![])
            }
        }

        async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> anyhow::Result<()> {
            Ok(self.socket.send(Message::Bytes(Vec::from(buf))).await?)
        }
    }
}
