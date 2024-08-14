use tokio::join;
use tokio::sync::mpsc;

mod database;
mod database_manager;
mod wager_manager;
mod connection_manager;

use database::*;
use database_manager::DatabaseManager;
use wager_manager::WagerManager;

#[tokio::main]
async fn main() {
    let mut database = DatabaseConnection::new("127.0.0.1:8000").await.unwrap();

    let _ = generate_test_data(&mut database).await;

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
        connection_manager::hande_listen_server(db_tx, wager_tx).await;
    });

    let (res1, res2, res3) = join!(db_task, wager_task, listen_server_task);
    res1.unwrap();
    res2.unwrap();
    res3.unwrap();
}

async fn generate_test_data(database_connection: &mut DatabaseConnection) -> anyhow::Result<()> {
    let user_id = database_connection.add_user(&DbUser::new("aidan", 2000)).await?;
    let user_id = if let Some(record) = user_id {
        record.id
    } else {
        database_connection.get_user_by_name("aidan").await?.unwrap().id // presumably user already exists if we get none. A fatal error if it doesn't exist here
    };

    let wager_id = database_connection.add_wager(&DbWager::new("test_wager1", "a test wager", 200)).await?;
    let wager_id = if let Some(record) = wager_id {
        record.id
    } else {
        database_connection.get_wager_by_name("test_wager1").await?.unwrap().id
    };

    let wager_option_id = database_connection.add_wager_option_db(&DbWagerOption::new("test_wager_option1", "a test wager option", wager_id)).await?;
    let wager_option_id = if let Some(record) = wager_option_id {
        record.id
    } else {
        database_connection.get_wager_option_by_name("test_wager_option_id").await?.unwrap().id
    };

    //we truly do not care if this doesn't work
    let bet_id = database_connection.add_bet_db(&DbBet::new(user_id, wager_option_id, 200)).await;
    if let Err(e) = bet_id {
        println!("{:?}", e);
    }
    Ok(())
}
