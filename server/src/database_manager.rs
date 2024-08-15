use surrealdb::Connection;
use surrealdb::sql::Thing;
use tokio::sync::{mpsc, oneshot};
use crate::database::{DatabaseConnection, DbUser};

pub type Responder<T> = oneshot::Sender<anyhow::Result<T>>;

pub enum DatabaseRequest {
    GetUser {
        name: String,
        responder: Responder<Option<DbUser>>,
    },
    GetAllWagerInfo {
        responder: Responder<Vec<common::Wager>>,
    },
    GetWagerInfo {
        id: Thing,
        responder: Responder<Option<common::Wager>>,
    },
    ProvidePayout {
        bet_info: common::Bet,
        winning_ratio: f64,
        responder: Responder<()>,
    },
}

pub struct DatabaseManager<Conn: Connection> {
    db_connection: DatabaseConnection<Conn>,
    work_queue: mpsc::Receiver<DatabaseRequest>,
}

pub fn transform_err<T>(error: surrealdb::Result<T>) -> anyhow::Result<T> {
    match error {
        Ok(t) => Ok(t),
        Err(e) => Err(e.into()),
    }
}

impl<Conn: Connection> DatabaseManager<Conn> {
    pub fn new(
        db_connection: DatabaseConnection<Conn>,
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
                    let resp = transform_err(self.db_connection.get_user_by_name(name.as_ref()).await);
                    let _ = responder.send(resp);
                }
                DatabaseRequest::GetAllWagerInfo { responder } => {
                    let resp = transform_err(self.db_connection.get_all_bet_info().await);
                    let _ = responder.send(resp);
                }
                DatabaseRequest::GetWagerInfo { id, responder } => {
                    let resp = transform_err(self.db_connection.get_info_for_wager(&id).await);
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
                    let _ = responder.send(transform_err(resp));
                }
            }
        }
    }
}
