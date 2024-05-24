use lan_bet::database::*;

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
    let mut db = DatabaseConnection::new("127.0.0.1:8000").await.unwrap();

    let user = User::new("test_user", 2000);
    db.add_user(&user).await?;

    let wager = Wager::new("test wager", "a test wager", 4000);
    db.add_wager(&wager).await?;

    let wager_option = WagerOption::new("option 1", "test option 1", wager.id.clone());
    db.add_wager_option(&wager_option).await?;

    let created_bet = db
        .add_bet(&Bet::new(user.id.clone(), wager_option.id.clone(), 400))
        .await?;
    dbg!(&created_bet);

    Ok(())
}
