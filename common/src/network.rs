use serde::{Deserialize, Serialize};
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
    pub async fn from_tcp_stream(connection: TcpStream) -> anyhow::Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let ws = not_wasm::TungsteniteWebSocket::new(connection).await?;

        #[cfg(target_arch = "wasm32")]
        let ws = wasm::WasmWebSocket::new("")?;

        Ok(Self {
            connection: ws,
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
    use std::future::Future;
    use std::task::Poll;
    use wasm_bindgen::prelude::*;
    use web_sys::{BinaryType, js_sys, MessageEvent, WebSocket};
    use crate::network:: WebSocketConnection;

    pub struct WasmWebSocket {
        socket: WebSocket,
        message_sender: async_channel::Sender<Vec<u8>>,
        message_receiver: async_channel::Receiver<Vec<u8>>,
    }

    impl WasmWebSocket {
        pub fn new(address: &str) -> anyhow::Result<Self> {
            let mut socket = WebSocket::new(address)?;
            socket.set_binary_type(BinaryType::Arraybuffer);
            let (message_sender, message_receiver) = async_channel::unbounded();
            
            let tx = message_sender.clone();
            let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
                if let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                    let array: Vec<u8> = buf.into();
                    let fut = tx.send(array);
                    while Poll::Pending = fut.poll() {}
                } else {}
            });
            socket.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
            onmessage_callback.forget();
            Ok(WasmWebSocket{ socket, message_receiver, message_sender })
        }
    }
    
    impl WebSocketConnection for WasmWebSocket {
        async fn read<'a>(&'a mut self) -> anyhow::Result<Vec<u8>> {
            Ok(self.message_receiver.recv().await?)
        }

        async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> anyhow::Result<()> {
            self.socket.send_with_u8_array(buf)?;
            Ok(())
        }
    }
}
