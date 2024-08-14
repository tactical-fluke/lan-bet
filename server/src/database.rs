use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use surrealdb::opt::auth::Root;
use surrealdb::sql::{Id, Thing};
use surrealdb::Surreal;
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    Result,
};

use surrealdb::sql::statements::BeginStatement;
use surrealdb::sql::statements::CommitStatement;

pub const TABLE_USER: &str = "user";
pub const TABLE_WAGER: &str = "wager";
pub const TABLE_WAGER_OPTION: &str = "wager_option";
pub const TABLE_BET: &str = "bet";

const GET_BY_NAME_QUERY: &str = "SELECT * FROM $table WHERE name = $name";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Record {
    #[allow(dead_code)]
    pub id: Thing,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DbUser {
    pub id: Thing,
    pub name: String,
    pub balance: u64,
}

impl DbUser {
    pub fn new(name: impl Into<String> + Clone, balance: u64) -> Self {
        Self {
            id: Thing {
                tb: TABLE_USER.into(),
                id: Id::String(name.clone().into()),
            },
            name: name.into(),
            balance,
        }
    }
}

impl Into<common::User> for DbUser {
    fn into(self) -> common::User {
        common::User {
            name: self.name,
            balance: self.balance,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DbWager {
    pub id: Thing,
    pub name: String,
    pub description: String,
    pub pot: u64,
    pub options: Vec<Thing>,
}

impl DbWager {
    pub fn new(name: impl Into<String>, description: impl Into<String>, pot: u64) -> Self {
        Self {
            id: Thing {
                tb: TABLE_WAGER.into(),
                id: Id::rand(),
            },
            name: name.into(),
            description: description.into(),
            pot,
            options: vec![],
        }
    }
}

impl Into<common::Wager> for DbWager {
    fn into(self) -> common::Wager {
        common::Wager {
            id: self.id.id.to_string(),
            name: self.name,
            description: self.description,
            pot: self.pot,
            options: vec![],
        }
    }
}

impl From<common::Wager> for DbWager {
    fn from(value: common::Wager) -> Self {
        Self::new(value.name, value.description, value.pot)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DbWagerOption {
    pub id: Thing,
    pub name: String,
    pub description: String,
    pub wager: Thing,
    pub bets: Vec<Thing>,
}

impl DbWagerOption {
    pub fn new(name: impl Into<String>, description: impl Into<String>, wager: Thing) -> Self {
        Self {
            id: Thing {
                tb: TABLE_WAGER_OPTION.into(),
                id: Id::rand(),
            },
            name: name.into(),
            description: description.into(),
            wager,
            bets: vec![],
        }
    }
}

impl Into<common::WagerOption> for DbWagerOption {
    fn into(self) -> common::WagerOption {
        common::WagerOption {
            id: self.id.id.to_string(),
            name: self.name,
            description: self.description,
            bets: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DbBet {
    pub id: Thing,
    pub user: Thing,
    pub wager_option: Thing,
    pub val: u64,
}

impl DbBet {
    pub fn new(user: Thing, wager_option: Thing, val: u64) -> Self {
        Self {
            id: Thing {
                tb: TABLE_BET.into(),
                id: Id::rand(),
            },
            user,
            wager_option,
            val,
        }
    }
}

impl Into<common::Bet> for DbBet {
    fn into(self) -> common::Bet {
        common::Bet {
            id: self.id.id.to_string(),
            user_id: self.id.id.to_string(),
            val: self.val,
        }
    }
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

    pub async fn add_user(&mut self, user: &DbUser) -> Result<Option<Record>> {
        Ok(self
            .connection
            .create(("user", user.id.clone()))
            .content(user)
            .await?)
    }

    pub async fn add_wager(&mut self, wager: &DbWager) -> Result<Option<Record>> {
        Ok(self.connection.create((TABLE_WAGER, wager.id.clone())).content(wager).await?)
    }

    pub async fn add_wager_option(&mut self, option: &common::WagerOption, wager_id: &str) -> Result<Option<Record>> {
        let wager_id = Thing{ tb: TABLE_WAGER.into(), id: Id::String(wager_id.into()) };
        let wager_option = DbWagerOption::new(&option.name, &option.description, wager_id);

        self.add_wager_option_db(&wager_option).await
    }

    pub async fn add_wager_option_db(&mut self, option: &DbWagerOption) -> Result<Option<Record>> {
        let mut response = self.connection
            .query(BeginStatement)
            .query("CREATE $id SET name = $name, description = $description, wager = $wager, bets = $bets")
            .bind(&option)
            .query("UPDATE $wager SET options = array::add($wager.options, $id);")
            .bind(("id", &option.id))
            .bind(("wager", &option.wager))
            .query(CommitStatement)
            .await?;

        Ok(response.take(1)?)
    }

    pub async fn add_bet(&mut self, bet: &common::Bet, wager_option_id: &str) -> Result<Option<Record>> {
        let wager_option_id = Thing{ tb: "wager_option".into(), id: Id::String(wager_option_id.into())};
        let user_id = Thing{ tb: "user".into(), id: Id::String(bet.user_id.clone())};
        let bet = DbBet::new(user_id, wager_option_id, bet.val);

        self.add_bet_db(&bet).await
    }

    pub async fn add_bet_db(&mut self, bet: &DbBet) -> Result<Option<Record>> {
        let mut response = self.connection
            .query(BeginStatement)
            .query("CREATE $id SET user = $user, wager_option = $wager_option, val = $val;")
            .bind(&bet)
            .query("UPDATE $wager_option SET bets = array::add($wager_option.bets, $id);")
            .bind(("id", &bet.id))
            .bind(("wager_option", &bet.wager_option))
            .query(CommitStatement)
            .await?;

        Ok(response.take(1)?)
    }

    pub async fn remove_wager(&mut self, wager_id: &Thing) -> Result<()> {
        let wager: DbWager = self.connection.select(wager_id).await?.unwrap();
        for option in &wager.options {
            self.remove_wager_option(option).await?;
        }
        let _: Option<DbWager> = self.connection.delete(wager_id).await?;
        Ok(())
    }

    pub async fn remove_wager_option(&mut self, option_id: &Thing) -> Result<()> {
        let option: DbWagerOption = self.connection.select(option_id).await?.unwrap();

        for bet in &option.bets {
            self.remove_bet(&bet).await?;
        }
        self.connection
            .query("UPDATE $wager SET options = array::remove(array::find_index($option_id));")
            .bind(("wager", &option.wager))
            .bind(("option_id", option_id))
            .await?;
        let _: Option<DbWagerOption> = self.connection.delete(option_id).await?;
        Ok(())
    }

    pub async fn remove_bet(&mut self, bet_id: &Thing) -> Result<()> {
        let bet: DbBet = self.connection.select(bet_id).await?.unwrap();
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

    pub async fn get_user_by_name(&self, name: &str) -> Result<Option<DbUser>> {
        self.get_item_by_name(TABLE_USER, name).await
    }

    pub async fn get_bets_by_user(&mut self, name: &str) -> Result<Vec<DbBet>> {
        self.connection
            .query("SELECT * FROM $table WHERE user = $name")
            .bind(("table", TABLE_USER))
            .bind(("name", name))
            .await?
            .take(0)
    }

    pub async fn get_all_wagers(&self) -> Result<Vec<DbWager>> {
        self.connection.select("wager").await
    }

    pub async fn get_wager_by_name(&self, name: impl Into<&str>) -> Result<Option<DbWager>> {
        self.get_item_by_name(TABLE_WAGER, name).await
    }

    pub async fn get_all_wager_options_for_wager(
        &self,
        wager_id: &str,
    ) -> Result<Vec<DbWagerOption>> {
        let constructed_id = Thing {
            tb: TABLE_WAGER.into(),
            id: wager_id.into(),
        };
        self.connection
            .query("SELECT * FROM wager_option WHERE wager = $wager_id;")
            .bind(("wager_id", constructed_id))
            .await?
            .take(0)
    }

    pub async fn get_wager_option_by_name(&self, name: impl Into<&str>) -> Result<Option<DbWagerOption>> {
        self.get_item_by_name(TABLE_WAGER_OPTION, name).await
    }

    pub async fn get_all_bets_for_wager_option(&self, option_id: &str) -> Result<Vec<DbBet>> {
        let constructed_id = Thing {
            tb: TABLE_BET.into(),
            id: option_id.into(),
        };
        self.connection
            .query("SELECT * FROM bet WHERE wager_option = $option_id;")
            .bind(("option_id", constructed_id))
            .await?
            .take(0)
    }

    pub async fn get_all_bet_info(&self) -> Result<Vec<common::Wager>> {
        let mut response = self
            .connection
            .query("SELECT * FROM wager FETCH options, options.bets")
            .await?;
        response.take(0)
    }

    pub async fn get_info_for_wager(&self, wager_id: Thing) -> Result<Option<common::Wager>> {
        assert_eq!(wager_id.tb, TABLE_WAGER);
        let mut response = self.connection.query("SELECT * FROM wager WHERE id = $id FETCH options, options.bets")
            .bind(("id", &wager_id))
            .await?;
        response.take(0)
    }

    pub async fn provide_payout_for_bet(&mut self, bet_info: &common::Bet, winning_ratio: f64) -> Result<()> {
        let user_id = Thing {
            tb: TABLE_USER.into(),
            id: Id::String(bet_info.user_id.clone())
        };
        self.connection.query("UPDATE $winner SET balance += $bet_value * $winning_ratio")
            .bind(("winner", user_id))
            .bind(("bet_value", &bet_info.val))
            .bind(("winning_ratio", winning_ratio))
            .await?;
        Ok(())
    }

    async fn get_item_by_name<Item: DeserializeOwned>(&self, table: impl Into<&str>, name: impl Into<&str>) -> Result<Option<Item>> {
        self.connection
            .query(GET_BY_NAME_QUERY)
            .bind(("table", table.into()))
            .bind(("name", name.into()))
            .await?
            .take(0)
    }
}
