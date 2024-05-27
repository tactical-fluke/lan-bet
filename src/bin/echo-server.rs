use lan_bet::database::*;
use lan_bet::network::*;
use std::collections::HashMap;
use std::io::ErrorKind;
use surrealdb::sql::Thing;
use surrealdb::Result;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

type Responder<T> = oneshot::Sender<Result<T>>;

enum DatabaseRequest {
    GetUser {
        name: String,
        responder: Responder<Option<User>>,
    },
    GetAllWagerInfo {
        responder: Responder<Vec<WagerInfo>>,
    },
    GetWagerInfo {
        id: Thing,
        responder: Responder<Option<WagerInfo>>,
    },
    ProvidePayout {
        bet_info: BetInfo,
        winning_ratio: f64,
        responder: Responder<()>,
    },
}

#[tokio::main]
async fn main() {
    let mut database = DatabaseConnection::new("127.0.0.1:8000").await.unwrap();
    let _ = database.add_user(&User::new("aidan", 2000)).await; // do not care about failure, as the user could already have been created
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
                let _ = responder.send(resp);
            }
            DatabaseRequest::GetAllWagerInfo { responder } => {
                let resp = db.get_all_bet_info().await;
                let _ = responder.send(resp);
            }
            DatabaseRequest::GetWagerInfo { id, responder } => {
                let resp = db.get_info_for_wager(id).await;
                let _ = responder.send(resp);
            }
            DatabaseRequest::ProvidePayout {
                bet_info,
                winning_ratio,
                responder,
            } => {
                let resp = db.provide_payout_for_bet(&bet_info, winning_ratio).await;
                let _ = responder.send(resp);
            }
        }
    }
}

async fn resolve_wager(
    tx: mpsc::Sender<DatabaseRequest>,
    wager_id: Thing,
    winning_option_id: Thing,
) -> Result<()> {
    let (wager_tx, wager_rx) = oneshot::channel();
    tx.send(DatabaseRequest::GetWagerInfo {
        id: wager_id,
        responder: wager_tx,
    })
    .await
    .unwrap();

    let wager_info = wager_rx.await.unwrap()?.unwrap();
    let mut wager_total_map = HashMap::new();
    for option in &wager_info.options {
        let total: u64 = option.bets.iter().map(|bet| bet.val).sum();
        wager_total_map.insert(option.id.clone(), total);
    }

    let abs_total: u64 =
        wager_total_map.iter().map(|(_, value)| value).sum::<u64>() + wager_info.pot;
    let winning_ratio = abs_total as f64 / *wager_total_map.get(&winning_option_id).unwrap() as f64;

    let winning_bets = &wager_info
        .options
        .iter()
        .find(|option| option.id == winning_option_id)
        .unwrap()
        .bets;

    for winning_bet in winning_bets {
        let (payout_tx, payout_rx) = oneshot::channel();
        tx.send(DatabaseRequest::ProvidePayout {
            bet_info: winning_bet.clone(),
            winning_ratio,
            responder: payout_tx,
        })
        .await
        .unwrap();
        payout_rx.await.unwrap()?;
    }
    Ok(())
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
    if let Packet::RequestPacket(request) = packet {
        match request {
            Request::Login { user } => {
                let (resp_tx, resp_rx) = oneshot::channel();

                let req = DatabaseRequest::GetUser {
                    name: user.clone(),
                    responder: resp_tx,
                };
                tx.send(req).await.unwrap();
                let response = resp_rx.await;
                let response = response.unwrap();
                let user = response.unwrap().ok_or(tokio::io::Error::new(
                    ErrorKind::NotFound,
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
                        break; //re-login, no
                    }
                    Request::WhoAmI => {
                        connection
                            .send(Packet::ResponsePacket(Response::WhoAmI(username.clone())))
                            .await
                            .unwrap();
                    }
                    Request::WagerData => {
                        let (db_tx, db_rx) = oneshot::channel();
                        tx.send(DatabaseRequest::GetAllWagerInfo { responder: db_tx })
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
                    Request::ResolveWager {
                        wager_id,
                        winning_option_id,
                    } => {
                        let result = resolve_wager(tx.clone(), wager_id, winning_option_id).await;
                        if let Ok(()) = result {
                            connection
                                .send(Packet::ResponsePacket(Response::None))
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
