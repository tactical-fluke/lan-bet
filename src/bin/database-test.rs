use lan_bet::database::*;

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
    let mut db = DatabaseConnection::new("127.0.0.1:8000").await.unwrap();

    let created_user = db
        .add_user(User {
            name: "test_name".into(),
            balance: 2000,
        })
        .await?;
    dbg!(&created_user);

    let created_wager = db
        .add_wager(Wager {
            name: "test wager".into(),
            description: "a test wager".into(),
            pot: 4000,
            pool: None,
            options: vec![],
        })
        .await?;
    dbg!(&created_wager);

    let created_wager_option = db
        .add_wager_option(WagerOption {
            name: "option 1".into(),
            description: "test option 1".into(),
            pool: None,
            wager: created_wager.id.clone(),
            bets: vec![],
        })
        .await?;
    dbg!(&created_wager_option);

    let created_bet = db
        .add_bet(Bet {
            user: created_user.id.clone(),
            wager_option: created_wager.id.clone(),
            val: 400,
        })
        .await?;
    dbg!(&created_bet);

    Ok(())
}
