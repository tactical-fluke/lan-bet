use lan_bet::database::*;
use lan_bet::network::*;
use std::collections::HashMap;
use std::io::ErrorKind;
use surrealdb::sql::Thing;
use surrealdb::Result;
use tokio::join;
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

struct DatabaseManager {
    db_connection: DatabaseConnection,
    work_queue: mpsc::Receiver<DatabaseRequest>,
}

impl DatabaseManager {
    pub fn new(
        db_connection: DatabaseConnection,
        work_queue: mpsc::Receiver<DatabaseRequest>,
    ) -> Self {
        Self {
            db_connection,
            work_queue,
        }
    }

    pub async fn manage(&mut self) {
        while let Some(request) = self.work_queue.recv().await {
            match request {
                DatabaseRequest::GetUser { name, responder } => {
                    let resp = self.db_connection.get_user(&name).await;
                    let _ = responder.send(resp);
                }
                DatabaseRequest::GetAllWagerInfo { responder } => {
                    let resp = self.db_connection.get_all_bet_info().await;
                    let _ = responder.send(resp);
                }
                DatabaseRequest::GetWagerInfo { id, responder } => {
                    let resp = self.db_connection.get_info_for_wager(id).await;
                    let _ = responder.send(resp);
                }
                DatabaseRequest::ProvidePayout {
                    bet_info,
                    winning_ratio,
                    responder,
                } => {
                    let resp = self
                        .db_connection
                        .provide_payout_for_bet(&bet_info, winning_ratio)
                        .await;
                    let _ = responder.send(resp);
                }
            }
        }
    }
}

enum WagerRequest {
    ResolveWager {
        wager_id: Thing,
        winning_option: Thing,
        responder: Responder<()>,
    },
}

struct WagerManager {
    work_queue: mpsc::Receiver<WagerRequest>,
    database_requester: mpsc::Sender<DatabaseRequest>,
}

impl WagerManager {
    pub fn new(
        work_queue: mpsc::Receiver<WagerRequest>,
        database_requester: mpsc::Sender<DatabaseRequest>,
    ) -> Self {
        Self {
            work_queue,
            database_requester,
        }
    }

    pub async fn manage(&mut self) {
        while let Some(request) = self.work_queue.recv().await {
            match request {
                WagerRequest::ResolveWager {
                    wager_id,
                    winning_option,
                    responder,
                } => {
                    responder
                        .send(self.resolve_wager(wager_id, winning_option).await)
                        .unwrap();
                }
            }
        }
    }

    async fn resolve_wager(&mut self, wager_id: Thing, winning_option_id: Thing) -> Result<()> {
        let (wager_tx, wager_rx) = oneshot::channel();
        self.database_requester
            .send(DatabaseRequest::GetWagerInfo {
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
        let winning_ratio =
            abs_total as f64 / *wager_total_map.get(&winning_option_id).unwrap() as f64;

        let winning_bets = &wager_info
            .options
            .iter()
            .find(|option| option.id == winning_option_id)
            .unwrap()
            .bets;

        for winning_bet in winning_bets {
            let (payout_tx, payout_rx) = oneshot::channel();
            self.database_requester
                .send(DatabaseRequest::ProvidePayout {
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
}

#[tokio::main]
async fn main() {
    let mut database = DatabaseConnection::new("127.0.0.1:8000").await.unwrap();
    let _ = database.add_user(&User::new("aidan", 2000)).await; // do not care about failure, as the user could already have been created
    let (db_tx, db_rx) = mpsc::channel(32);
    let mut db_manager = DatabaseManager::new(database, db_rx);

    let db_task = tokio::spawn(async move {
        db_manager.manage().await;
    });

    let (wager_tx, wager_rx) = mpsc::channel(32);
    let mut wager_manager = WagerManager::new(wager_rx, db_tx.clone());

    let wager_task = tokio::spawn(async move {
        wager_manager.manage().await;
    });

    let listen_server_task = tokio::spawn(async move {
        hande_listen_server(db_tx, wager_tx).await;
    });

    let (res1, res2, res3) = join!(db_task, wager_task, listen_server_task);
    res1.unwrap();
    res2.unwrap();
    res3.unwrap();
}

async fn hande_listen_server(db_tx: mpsc::Sender<DatabaseRequest>, wager_tx: mpsc::Sender<WagerRequest>) {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let (connection, _) = listener.accept().await.unwrap();
        let tx = db_tx.clone();
        let wage_tx = wager_tx.clone();

        tokio::spawn(async move {
            let connection = Connection::from_tcp_stream(connection);
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
        connection
            .send(Packet::ResponsePacket(Response::None))
            .await
            .unwrap();
        dbg!("moving to handle client");
        handle_client(username, connection, db_tx, wager_tx).await;
    } else {
        connection.send(Packet::Error).await.unwrap();
    }
}

async fn handle_login(
    connection: &mut Connection,
    db_tx: &mut mpsc::Sender<DatabaseRequest>,
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
                db_tx.send(req).await.unwrap();
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
    db_tx: mpsc::Sender<DatabaseRequest>,
    wager_tx: mpsc::Sender<WagerRequest>,
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
                        let (resp_tx, resp_rx) = oneshot::channel();
                        db_tx
                            .send(DatabaseRequest::GetAllWagerInfo { responder: resp_tx })
                            .await
                            .unwrap();
                        let response = resp_rx.await.unwrap();
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
                        let (resp_tx, resp_rx) = oneshot::channel();
                        wager_tx
                            .send(WagerRequest::ResolveWager {
                                wager_id,
                                winning_option: winning_option_id,
                                responder: resp_tx,
                            })
                            .await
                            .unwrap();
                        let result = resp_rx.await.unwrap();
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
