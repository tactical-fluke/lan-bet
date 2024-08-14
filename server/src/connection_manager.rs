use tokio::sync::{mpsc, oneshot};
use tokio::net::TcpListener;
use common::network::{Connection, Packet, Request, Response};
use std::io::ErrorKind;
use anyhow::{anyhow, bail};
use surrealdb::sql::{Id, Thing};
use crate::database_manager::DatabaseRequest;
use crate::wager_manager::WagerRequest;

pub async fn hande_listen_server(
    db_tx: mpsc::Sender<DatabaseRequest>,
    wager_tx: mpsc::Sender<WagerRequest>,
) {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let (connection, _) = listener.accept().await.unwrap();
        let tx = db_tx.clone();
        let wage_tx = wager_tx.clone();

        tokio::spawn(async move {
            let connection = Connection::from_tcp_stream(connection).await.unwrap();
            handle_connection(connection, tx, wage_tx).await;
        });
    }
}

async fn handle_connection(
    mut connection: Connection,
    mut db_tx: mpsc::Sender<DatabaseRequest>,
    wager_tx: mpsc::Sender<WagerRequest>,
) {
    let user = handle_login(&mut connection, &mut db_tx).await;
    if let Ok(username) = user {
        match handle_client(username, &mut connection, db_tx, wager_tx).await {
            Ok(()) => {}
            Err(_) => {
                connection.send(Packet::Error).await.unwrap();
            }
        }
    } else {
        connection.send(Packet::Error).await.unwrap();
    }
}

async fn handle_login(
    connection: &mut Connection,
    db_tx: &mut mpsc::Sender<DatabaseRequest>,
) -> anyhow::Result<String> {
    let packet = connection.read().await?;
    if let Packet::RequestPacket(request) = packet {
        match request {
            Request::Login { user } => {
                let (resp_tx, resp_rx) = oneshot::channel();

                let req = DatabaseRequest::GetUser {
                    name: user.clone(),
                    responder: resp_tx,
                };
                db_tx.send(req).await.unwrap();
                let response = resp_rx.await?;
                let user = response?.ok_or(std::io::Error::new(
                    ErrorKind::NotFound,
                    "no such user found",
                ))?;
                connection
                    .send(Packet::ResponsePacket(Response::SuccessfulLogin {
                        username: user.name.clone(),
                        balance: user.balance,
                    }))
                    .await?;
                Ok(user.name)
            }
            _ => {
                bail!("bad login");
            }
        }
    } else {
        bail!("Invalid request at login: {:?}", packet);
    }
}

async fn handle_client(
    username: String,
    connection: &mut Connection,
    db_tx: mpsc::Sender<DatabaseRequest>,
    wager_tx: mpsc::Sender<WagerRequest>,
) -> anyhow::Result<()> {
    loop {
        let packet = connection.read().await;
        if let Ok(Packet::RequestPacket(request)) = packet {
            match request {
                Request::Login { user: _ } => {
                    dbg!("duplicate login detected!");
                    connection.send(Packet::Error).await.unwrap();
                    bail!("Attempted re-login - denied");
                }
                Request::WhoAmI => {
                    connection
                        .send(Packet::ResponsePacket(Response::WhoAmI(username.clone())))
                        .await?;
                }
                Request::WagerData => {
                    let (resp_tx, resp_rx) = oneshot::channel();
                    db_tx
                        .send(DatabaseRequest::GetAllWagerInfo { responder: resp_tx })
                        .await?;
                    let response = resp_rx.await?;
                    if let Ok(wager_info) = response {
                        connection
                            .send(Packet::ResponsePacket(Response::WagerData(wager_info)))
                            .await?;
                    } else {
                        connection.send(Packet::Error).await?;
                    }
                }
                Request::ResolveWager {
                    wager_id,
                    winning_option_id,
                } => {
                    let (resp_tx, resp_rx) = oneshot::channel();
                    wager_tx
                        .send(WagerRequest::ResolveWager {
                            wager_id: Thing {
                                tb: "wager".into(),
                                id: Id::String(wager_id),
                            },
                            winning_option: Thing {
                                tb: "wager_option".into(),
                                id: Id::String(winning_option_id),
                            },
                            responder: resp_tx,
                        })
                        .await?;
                    let result = resp_rx.await?;
                    if let Ok(()) = result {
                        connection
                            .send(Packet::ResponsePacket(Response::None))
                            .await?;
                    } else {
                        connection.send(Packet::Error).await?;
                    }
                }
            }
        } else {
            return match packet {
                Ok(pack) => bail!("incorrect packet type: {:?}", pack),
                Err(error) => {
                    match &error.downcast_ref::<std::io::Error>().ok_or(anyhow!("not an std error"))?.kind() {
                        ErrorKind::ConnectionAborted => Ok(()), //connection aborted is considered successful,
                        _ => Err(error)?,
                    }
                }
            };
        }
    }
}
