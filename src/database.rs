use serde::{Deserialize, Serialize};
use surrealdb::opt::auth::Root;
use surrealdb::sql::{Id, Thing};
use surrealdb::Surreal;
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    Result,
};

use surrealdb::sql::statements::BeginStatement;
use surrealdb::sql::statements::CommitStatement;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Record {
    #[allow(dead_code)]
    pub id: Thing,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: Thing,
    pub name: String,
    pub balance: u64,
}

impl User {
    pub fn new(name: impl Into<String> + Clone, balance: u64) -> Self {
        Self {
            id: Thing {
                tb: "user".into(),
                id: Id::String(name.clone().into()),
            },
            name: name.into(),
            balance,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Wager {
    pub id: Thing,
    pub name: String,
    pub description: String,
    pub pot: u64,
    pub options: Vec<Thing>,
}

impl Wager {
    pub fn new(name: impl Into<String>, description: impl Into<String>, pot: u64) -> Self {
        Self {
            id: Thing {
                tb: "wager".into(),
                id: Id::rand(),
            },
            name: name.into(),
            description: description.into(),
            pot,
            options: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WagerOption {
    pub id: Thing,
    pub name: String,
    pub description: String,
    pub wager: Thing,
    pub bets: Vec<Thing>,
}

impl WagerOption {
    pub fn new(name: impl Into<String>, description: impl Into<String>, wager: Thing) -> Self {
        Self {
            id: Thing {
                tb: "wager_option".into(),
                id: Id::rand(),
            },
            name: name.into(),
            description: description.into(),
            wager,
            bets: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Bet {
    pub id: Thing,
    pub user: Thing,
    pub wager_option: Thing,
    pub val: u64,
}

impl Bet {
    pub fn new(user: Thing, wager_option: Thing, val: u64) -> Self {
        Self {
            id: Thing {
                tb: "bet".into(),
                id: Id::rand(),
            },
            user,
            wager_option,
            val,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct WagerInfo {
    pub id: Thing,
    pub name: String,
    pub description: String,
    pub pot: u64,
    pub options: Vec<WagerOptionInfo>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct WagerOptionInfo {
    pub id: Thing,
    pub name: String,
    pub description: String,
    pub bets: Vec<BetInfo>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BetInfo {
    pub id: Thing,
    pub user: Thing,
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

    pub async fn add_user(&mut self, user: &User) -> Result<()> {
        let _: Option<Record> = self
            .connection
            .create(("user", &user.name))
            .content(user)
            .await?;

        Ok(())
    }

    pub async fn add_wager(&mut self, wager: &Wager) -> Result<()> {
        let _: Vec<Record> = self.connection.create("wager").content(wager).await?;

        Ok(())
    }

    pub async fn add_wager_option(&mut self, option: &WagerOption) -> Result<()> {
        self.connection
            .query(BeginStatement)
            .query("CREATE $id SET name = $name, description = $description, wager = $wager, bets = $bets")
            .bind(&option)
            .query("UPDATE $wager SET options = array::add($wager.options, $id);")
            .bind(("id", &option.id))
            .bind(("wager", &option.wager))
            .query(CommitStatement)
            .await?;
        Ok(())
    }

    pub async fn add_bet(&mut self, bet: &Bet) -> Result<()> {
        self.connection
            .query(BeginStatement)
            .query("CREATE $id SET user = $user, wager_option = $option, val = $val;")
            .bind(&bet)
            .query("UPDATE $wager_option SET bets = array::add($wager_option.bets, $id);")
            .bind(("id", &bet.id))
            .bind(("wager_option", &bet.wager_option))
            .query(CommitStatement)
            .await?;
        Ok(())
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
            .query(BeginStatement)
            .query("UPDATE $wager_option SET bets = array::remove(array::find_index($id));")
            .bind(("id", &bet.id))
            .bind(("wager_option", &bet.wager_option))
            .query("UPDATE $user SET balance += $val;")
            .bind(("user", &bet.user))
            .bind(("val", &bet.val))
            .query("DELETE $bet;")
            .bind(("bet", &bet.id))
            .query(CommitStatement)
            .await?;

        Ok(())
    }

    pub async fn get_user(&self, name: &str) -> Result<Option<User>> {
        self.connection.select(("user", name)).await
    }

    pub async fn get_bets_by_user(&mut self, username: &str) -> Result<Vec<Bet>> {
        let constructed_id = Thing {
            tb: "user".into(),
            id: username.into(),
        };
        self.connection
            .query("SELECT * from bet WHERE user = $user_id;")
            .bind(("user_id", constructed_id))
            .await?
            .take(0)
    }

    pub async fn get_all_wagers(&self) -> Result<Vec<Wager>> {
        self.connection.select("wager").await
    }

    pub async fn get_all_wager_options_for_wager(
        &self,
        wager_id: &str,
    ) -> Result<Vec<WagerOption>> {
        let constructed_id = Thing {
            tb: "wager".into(),
            id: wager_id.into(),
        };
        self.connection
            .query("SELECT * FROM wager_option WHERE wager = $wager_id;")
            .bind(("wager_id", constructed_id))
            .await?
            .take(0)
    }

    pub async fn get_all_bets_for_wager_option(&self, option_id: &str) -> Result<Vec<Bet>> {
        let constructed_id = Thing {
            tb: "bet".into(),
            id: option_id.into(),
        };
        self.connection
            .query("SELECT * FROM bet WHERE wager_option = $option_id;")
            .bind(("option_id", constructed_id))
            .await?
            .take(0)
    }

    pub async fn get_all_bet_info(&self) -> Result<Vec<WagerInfo>> {
        let mut response = self
            .connection
            .query("SELECT * FROM wager FETCH options, options.bets")
            .await?;
        response.take(0)
    }

    pub async fn get_info_for_wager(&self, wager_id: Thing) -> Result<Option<WagerInfo>> {
        assert_eq!(wager_id.tb, "wager");
        let mut response = self.connection.query("SELECT * FROM wager WHERE id = $id FETCH options, options.bets")
            .bind(("id", &wager_id))
            .await?;
        response.take(0)
    }

    pub async fn provide_payout_for_bet(&mut self, bet_info: &BetInfo, winning_ratio: f64) -> Result<()> {
        self.connection.query("UPDATE $winner SET balance += $bet_value * $winning_ratio")
            .bind(("winner", &bet_info.user))
            .bind(("bet_value", &bet_info.val))
            .bind(("winning_ratio", winning_ratio))
            .await?;
        Ok(())
    }
}
