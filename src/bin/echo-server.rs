use lan_bet::database::*;
use lan_bet::network::*;
use std::io::ErrorKind;
use surrealdb::Result;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

enum DatabaseRequest {
    GetUser {
        name: String,
        responder: oneshot::Sender<Option<User>>,
    },
    GetWagerInfo {
        responder: oneshot::Sender<Result<Vec<WagerInfo>>>,
    },
}

#[tokio::main]
async fn main() {
    let mut database = DatabaseConnection::new("127.0.0.1:8000").await.unwrap();
    let _ = database
        .add_user(User {
            name: "aidan".into(),
            balance: 2000,
        })
        .await; // do not care about failure, as the user could already have been created
    let (db_tx, db_rx) = mpsc::channel(32);

    tokio::spawn(async move {
        manage_database(database, db_rx).await;
    });

    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let (connection, _) = listener.accept().await.unwrap();
        let tx = db_tx.clone();

        tokio::spawn(async move {
            let connection = Connection::from_tcp_stream(connection);
            handle_connection(connection, tx).await;
        });
    }
}

async fn manage_database(mut db: DatabaseConnection, mut rx: mpsc::Receiver<DatabaseRequest>) {
    while let Some(request) = rx.recv().await {
        match request {
            DatabaseRequest::GetUser { name, responder } => {
                let resp = db.get_user(&name).await;
                let _ = responder.send(resp.unwrap());
            }
            DatabaseRequest::GetWagerInfo { responder } => {
                let resp = db.get_all_bet_info().await;
                let _ = responder.send(resp);
            }
        }
    }
}

async fn handle_connection(mut connection: Connection, mut tx: mpsc::Sender<DatabaseRequest>) {
    let user = handle_login(&mut connection, &mut tx).await;
    if let Ok(username) = user {
        connection
            .send(Packet::ResponsePacket(Response::None))
            .await
            .unwrap();
        dbg!("moving to handle client");
        handle_client(username, connection, &mut tx).await;
    } else {
        connection.send(Packet::Error).await.unwrap();
    }
}

async fn handle_login(
    connection: &mut Connection,
    tx: &mut mpsc::Sender<DatabaseRequest>,
) -> tokio::io::Result<String> {
    let packet = connection.read().await?;
    dbg!(&packet);
    if let Packet::RequestPacket(request) = packet {
        match request {
            Request::Login { user } => {
                let (resp_tx, resp_rx) = oneshot::channel();

                let req = DatabaseRequest::GetUser {
                    name: user.clone(),
                    responder: resp_tx,
                };
                let db_response = tx.send(req).await;
                dbg!(&db_response);
                let response = resp_rx.await;
                dbg!(&response);
                let response = response.unwrap();
                let user = response.ok_or(tokio::io::Error::new(
                    tokio::io::ErrorKind::NotFound,
                    "no such error found",
                ))?;
                Ok(user.name)
            }
            _ => {
                let error = tokio::io::Error::new(ErrorKind::Other, "bad login");
                Err(error)
            }
        }
    } else {
        Err(tokio::io::Error::new(
            ErrorKind::InvalidData,
            "invalid request at login",
        ))
    }
}

async fn handle_client(
    username: String,
    mut connection: Connection,
    tx: &mut mpsc::Sender<DatabaseRequest>,
) {
    loop {
        let packet = connection.read().await;
        dbg!(&packet);
        if let Ok(packet) = packet {
            if let Packet::RequestPacket(request) = packet {
                match request {
                    Request::Login { user: _ } => {
                        dbg!("duplicate login detected!");
                        connection.send(Packet::Error).await.unwrap();
                        break; //relogin, no
                    }
                    Request::WhoAmI => {
                        connection
                            .send(Packet::ResponsePacket(Response::WhoAmI(username.clone())))
                            .await
                            .unwrap();
                    }
                    Request::WagerData => {
                        let (db_tx, db_rx) = oneshot::channel();
                        tx.send(DatabaseRequest::GetWagerInfo { responder: db_tx })
                            .await
                            .unwrap();
                        let response = db_rx.await.unwrap();
                        if let Ok(wager_info) = response {
                            connection
                                .send(Packet::ResponsePacket(Response::WagerData(wager_info)))
                                .await
                                .unwrap();
                        } else {
                            connection.send(Packet::Error).await.unwrap();
                        }
                    }
                }
            }
        } else {
            eprintln!("{:?}", packet);
            break;
        }
    }
}
