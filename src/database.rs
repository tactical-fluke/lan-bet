use serde::{Deserialize, Serialize};
use surrealdb::opt::auth::Root;
use surrealdb::sql::Thing;
use surrealdb::Surreal;
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    Result,
};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Record {
    #[allow(dead_code)]
    pub id: Thing,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub name: String,
    pub balance: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Wager {
    pub name: String,
    pub description: String,
    pub pot: u64,
    pub pool: Option<u64>,
    pub options: Vec<Thing>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WagerOption {
    pub name: String,
    pub description: String,
    pub pool: Option<u64>,
    pub wager: Thing,
    pub bets: Vec<Thing>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Bet {
    pub user: Thing,
    pub wager_option: Thing,
    pub val: u64,
}

pub struct DatabaseConnection {
    connection: Surreal<Client>,
}

impl DatabaseConnection {
    pub async fn new(address: &str) -> Option<Self> {
        let db = Surreal::new::<Ws>(address).await.ok()?;

        db.signin(Root {
            username: "root",
            password: "root",
        })
        .await
        .ok()?;

        db.use_ns("test").use_db("lan_bet").await.ok()?;

        Some(Self { connection: db })
    }

    pub async fn add_user(&mut self, user: User) -> Result<Record> {
        let res: Option<Record> = self
            .connection
            .create(("user", &user.name))
            .content(user)
            .await?;

        Ok(res.unwrap())
    }

    pub async fn add_wager(&mut self, wager: Wager) -> Result<Record> {
        let mut res: Vec<Record> = self.connection.create("wager").content(wager).await?;

        // TODO if this fails, unwind and remove the wager
        self.connection.query("UPDATE $id SET pool = <future> { math::sum((SELECT pool FROM wager_option WHERE id = $id).pool)};")
            .bind(("id", res.first().unwrap().id.clone())).await?;
        Ok(res.pop().unwrap())
    }

    pub async fn add_wager_option(&mut self, option: WagerOption) -> Result<Record> {
        let mut res: Vec<Record> = self
            .connection
            .create("wager_option")
            .content(&option)
            .await?;

        // TODO if any of these fail, we should actually unwind, and delete the option
        self.connection.query("UPDATE $id SET pool = <future> { math::sum((SELECT val FROM bet WHERE id = $id).val)};")
            .bind(("id", res.first().unwrap().id.clone())).await?;
        self.connection.query("UPDATE $wager SET options = array::add((SELECT options FROM $wager).options, $id);")
            .bind(("id", res.first().unwrap().id.clone()))
            .bind(("wager", option.wager.clone())).await?;
        Ok(res.pop().unwrap())
    }

    pub async fn add_bet(&mut self, bet: Bet) -> Result<Record> {
        let mut res: Vec<Record> = self.connection.create("bet").content(&bet).await?;

        // TODO if this fails, unwind and delete the bet
        self.connection.query("UPDATE $wager_option SET bets = array::add((SELECT bets FROM $wager_option).bets, $id);")
            .bind(("id", res.first().unwrap().clone().id))
            .bind(("wager_option", bet.wager_option.clone())).await?;
        Ok(res.pop().unwrap())
    }

    pub async fn remove_wager(&mut self, wager_id: &Thing) -> Result<()> {
        let wager: Wager = self.connection.select(wager_id).await?.unwrap();
        for option in &wager.options {
            self.remove_wager_option(option).await?;
        }
        let _: Option<Wager> = self.connection.delete(wager_id).await?;
        Ok(())
    }

    pub async fn remove_wager_option(&mut self, option_id: &Thing) -> Result<()> {
        let option: WagerOption = self.connection.select(option_id).await?.unwrap();

        for bet in &option.bets {
            self.remove_bet(&bet).await?;
        }
        self.connection
            .query("UPDATE $wager SET options = array::remove(array::find_index($option_id));")
            .bind(("wager", &option.wager))
            .bind(("option_id", option_id))
            .await?;
        let _: Option<WagerOption> = self.connection.delete(option_id).await?;
        Ok(())
    }

    pub async fn remove_bet(&mut self, bet_id: &Thing) -> Result<()> {
        let bet: Bet = self.connection.select(bet_id).await?.unwrap();
        self.connection
            .query("UPDATE $wager_option SET bets = array::remove(array::find_index($id));")
            .bind(("id", bet_id.clone()))
            .bind(("wager_option", bet.wager_option.clone()))
            .await?;
        let mut user: User = self.connection.select(&bet.user).await?.unwrap();
        user.balance += bet.val;
        let _: Option<User> = self.connection.update(&bet.user).content(user).await?;
        let _: Option<Bet> = self.connection.delete(bet_id).await?;

        Ok(())
    }

    pub async fn get_user(&mut self, name: &str) -> Option<User> {
        self.connection.select(("user", name)).await.unwrap()
    }
}
