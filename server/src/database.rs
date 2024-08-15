use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use surrealdb::engine::local::{Db, Mem};
use surrealdb::opt::auth::Root;
use surrealdb::sql::{Id, Thing};
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    Result,
};
use surrealdb::{Connection, Surreal};

use surrealdb::sql::statements::BeginStatement;
use surrealdb::sql::statements::CommitStatement;

pub const TABLE_USER: &str = "user";
pub const TABLE_WAGER: &str = "wager";
pub const TABLE_WAGER_OPTION: &str = "wager_option";
pub const TABLE_BET: &str = "bet";

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Record {
    #[allow(dead_code)]
    pub id: Thing,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
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

pub struct DatabaseConnection<Type: Connection> {
    connection: Surreal<Type>,
}

impl DatabaseConnection<Client> {
    pub async fn new(address: &str) -> Result<Self> {
        let db = Surreal::new::<Ws>(address).await?;

        db.signin(Root {
            username: "root",
            password: "root",
        })
        .await?;

        db.use_ns("test").use_db("lan_bet").await?;

        Ok(Self { connection: db })
    }
}

impl DatabaseConnection<Db> {
    pub async fn new() -> Result<Self> {
        let connection = Surreal::new::<Mem>(()).await?;

        connection.use_ns("test").use_db("lan_bet").await?;

        Ok(Self { connection })
    }
}

impl<Type: Connection> DatabaseConnection<Type> {
    pub async fn select<FetchedType: DeserializeOwned>(
        &self,
        id: &Thing,
    ) -> Result<Option<FetchedType>> {
        self.connection.select(id).await
    }

    pub async fn add_user(&mut self, user: &DbUser) -> Result<Option<Record>> {
        debug_assert_eq!(&user.id.tb, &TABLE_USER.to_string());
        Ok(self
            .connection
            .create(("user", user.id.clone()))
            .content(user)
            .await?)
    }

    pub async fn add_wager(&mut self, wager: &DbWager) -> Result<Option<Record>> {
        debug_assert_eq!(&wager.id.tb, &TABLE_WAGER.to_string());
        Ok(self
            .connection
            .create((TABLE_WAGER, wager.id.clone()))
            .content(wager)
            .await?)
    }

    pub async fn add_wager_option(
        &mut self,
        option: &common::WagerOption,
        wager_id: &str,
    ) -> Result<Option<Record>> {
        let wager_id = Thing {
            tb: TABLE_WAGER_OPTION.into(),
            id: Id::String(wager_id.into()),
        };
        let wager_option = DbWagerOption::new(&option.name, &option.description, wager_id);

        self.add_wager_option_db(&wager_option).await
    }

    pub async fn add_wager_option_db(&mut self, option: &DbWagerOption) -> Result<Option<Record>> {
        debug_assert_eq!(&option.id.tb, &TABLE_WAGER_OPTION.to_string());
        let mut response = self.connection
            .query(BeginStatement)
            .query("CREATE $id SET name = $name, description = $description, wager = $wager, bets = $bets")
            .bind(&option)
            .query("UPDATE $wager SET options = array::add($wager.options, $id);")
            .bind(("id", &option.id))
            .bind(("wager", &option.wager))
            .query(CommitStatement)
            .await?;

        Ok(response.take(0)?)
    }

    pub async fn add_bet(
        &mut self,
        bet: &common::Bet,
        wager_option_id: &str,
    ) -> Result<Option<Record>> {
        let wager_option_id = Thing {
            tb: TABLE_WAGER_OPTION.into(),
            id: Id::String(wager_option_id.into()),
        };
        let user_id = Thing {
            tb: "user".into(),
            id: Id::String(bet.user_id.clone()),
        };
        let bet = DbBet::new(user_id, wager_option_id, bet.val);

        self.add_bet_db(&bet).await
    }

    pub async fn add_bet_db(&mut self, bet: &DbBet) -> Result<Option<Record>> {
        debug_assert_eq!(&bet.id.tb, &TABLE_BET.to_string());
        let mut response = self
            .connection
            .query(BeginStatement)
            .query("CREATE $id SET user = $user, wager_option = $wager_option, val = $val;")
            .bind(&bet)
            .query("UPDATE $wager_option SET bets = array::add($wager_option.bets, $id);")
            .bind(("id", &bet.id))
            .bind(("wager_option", &bet.wager_option))
            .query(CommitStatement)
            .await?;

        Ok(response.take(0)?)
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
            .query("UPDATE $wager SET options = array::remove($wager.options, array::find_index($wager.options, $option_id));")
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
            .query("UPDATE $wager_option SET bets = array::remove($wager_option.bets, array::find_index($wager_option.bets, $bet));")
            .bind(("bet", &bet.id))
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

    pub async fn get_user_by_name(&self, name: impl Into<&str>) -> Result<Option<DbUser>> {
        let mut response = self
            .connection
            .query("SELECT * FROM user WHERE name = $name")
            .bind(("name", name.into()))
            .await?;
        Ok(response.take(0)?)
    }

    pub async fn get_bets_by_user(&mut self, name: impl Into<&str>) -> Result<Vec<DbBet>> {
        let user_id = Thing {
            tb: TABLE_USER.into(),
            id: Id::String(name.into().to_string())
        };
        self.connection
            .query("SELECT * FROM bet WHERE user = $user_id")
            .bind(("user_id", user_id))
            .await?
            .take(0)
    }

    pub async fn get_all_wagers(&self) -> Result<Vec<DbWager>> {
        self.connection.select(TABLE_WAGER).await
    }

    pub async fn get_wager_by_name(&self, name: impl Into<&str>) -> Result<Option<DbWager>> {
        let mut response = self
            .connection
            .query("SELECT * FROM wager WHERE name = $name")
            .bind(("name", name.into()))
            .await?;
        Ok(response.take(0)?)
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

    pub async fn get_wager_option_by_name(
        &self,
        name: impl Into<&str>,
    ) -> Result<Option<DbWagerOption>> {
        let mut response = self
            .connection
            .query("SELECT * FROM wager_option WHERE name = $name")
            .bind(("name", name.into()))
            .await?;
        Ok(response.take(0)?)
    }

    pub async fn get_all_bets_for_wager_option(&self, option_id: &str) -> Result<Vec<DbBet>> {
        let constructed_id = Thing {
            tb: TABLE_WAGER_OPTION.into(),
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

    pub async fn get_info_for_wager(&self, wager_id: &Thing) -> Result<Option<common::Wager>> {
        assert_eq!(wager_id.tb, TABLE_WAGER);
        let mut response = self
            .connection
            .query("SELECT * FROM wager WHERE id = $id FETCH options, options.bets")
            .bind(("id", &wager_id))
            .await?;
        response.take(0)
    }

    pub async fn provide_payout_for_bet(
        &mut self,
        bet_info: &common::Bet,
        winning_ratio: f64,
    ) -> Result<()> {
        let user_id = Thing {
            tb: TABLE_USER.into(),
            id: Id::String(bet_info.user_id.clone()),
        };
        self.connection
            .query("UPDATE $winner SET balance += $bet_value * $winning_ratio")
            .bind(("winner", user_id))
            .bind(("bet_value", &bet_info.val))
            .bind(("winning_ratio", winning_ratio))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    pub async fn test_add_wager() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        db.add_wager(&DbWager::new("test", "test", 200))
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    pub async fn test_fetch_wager() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        let added_wager = db
            .add_wager(&DbWager::new("test", "test", 200))
            .await
            .unwrap()
            .unwrap();

        let fetched_wager = db.select::<DbWager>(&added_wager.id).await.unwrap().unwrap();
        assert_eq!(fetched_wager.name, "test".to_string());
        assert_eq!(fetched_wager.description, "test".to_string());
        assert_eq!(fetched_wager.pot, 200);
    }

    #[tokio::test]
    pub async fn test_fetch_wager_by_name() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        db.add_wager(&DbWager::new("test", "test", 200))
            .await
            .unwrap()
            .unwrap();

        let fetched_wager = db.get_wager_by_name("test").await.unwrap().unwrap();
        assert_eq!(fetched_wager.name, "test".to_string());
        assert_eq!(fetched_wager.description, "test".to_string());
        assert_eq!(fetched_wager.pot, 200);
    }

    #[tokio::test]
    pub async fn test_add_user() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        db.add_user(&DbUser::new("test user", 2000))
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    pub async fn test_fetch_user() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        let added_user = db
            .add_user(&DbUser::new("test_user", 2000))
            .await
            .unwrap()
            .unwrap();

        let fetched_user = db.select::<DbUser>(&added_user.id).await.unwrap().unwrap();
        assert_eq!(
            fetched_user.id,
            Thing {
                tb: TABLE_USER.into(),
                id: Id::String("test_user".into())
            }
        );
        assert_eq!(fetched_user.name, "test_user".to_string());
        assert_eq!(fetched_user.balance, 2000);
    }

    #[tokio::test]
    pub async fn test_fetch_user_by_name() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        db
            .add_user(&DbUser::new("test_user", 2000))
            .await
            .unwrap()
            .unwrap();

        let fetched_user = db.get_user_by_name("test_user").await.unwrap().unwrap();
        assert_eq!(
            fetched_user.id,
            Thing {
                tb: TABLE_USER.into(),
                id: Id::String("test_user".into())
            }
        );
        assert_eq!(fetched_user.name, "test_user".to_string());
        assert_eq!(fetched_user.balance, 2000);
    }

    #[tokio::test]
    pub async fn test_add_wager_option() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        let added_wager = db
            .add_wager(&DbWager::new("test", "test", 200))
            .await
            .unwrap()
            .unwrap();

        let added_wager_option = db
            .add_wager_option_db(&DbWagerOption::new(
                "test wager option",
                "a test wager option",
                added_wager.id.clone(),
            ))
            .await
            .unwrap()
            .unwrap();
        let fetched_wager = db
            .select::<DbWager>(&added_wager.id)
            .await
            .unwrap()
            .unwrap();
        assert!(fetched_wager.options.contains(&added_wager_option.id));
    }

    #[tokio::test]
    pub async fn test_fetch_wager_option() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        let added_wager = db
            .add_wager(&DbWager::new("test", "test", 200))
            .await
            .unwrap()
            .unwrap();
        let added_wager_option = db
            .add_wager_option_db(&DbWagerOption::new(
                "test wager option",
                "a test wager option",
                added_wager.id.clone(),
            ))
            .await
            .unwrap()
            .unwrap();

        let fetched_wager_option = db
            .select::<DbWagerOption>(&added_wager_option.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched_wager_option.wager, added_wager.id);
        assert_eq!(fetched_wager_option.name, "test wager option".to_string());
        assert_eq!(fetched_wager_option.bets, vec![]);
        assert_eq!(
            fetched_wager_option.description,
            "a test wager option".to_string()
        );
    }

    #[tokio::test]
    pub async fn test_fetch_wager_option_by_name() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        let added_wager = db
            .add_wager(&DbWager::new("test", "test", 200))
            .await
            .unwrap()
            .unwrap();

        db.add_wager_option_db(&DbWagerOption::new(
            "test_wager_option",
            "a test wager option",
            added_wager.id.clone(),
        ))
        .await
        .unwrap()
        .unwrap();
        let fetched_wager_option = db
            .get_wager_option_by_name("test_wager_option")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched_wager_option.wager, added_wager.id);
        assert_eq!(fetched_wager_option.name, "test_wager_option".to_string());
        assert_eq!(fetched_wager_option.bets, vec![]);
        assert_eq!(
            fetched_wager_option.description,
            "a test wager option".to_string()
        );
    }

    #[tokio::test]
    pub async fn test_add_bet() {
        let mut db = DatabaseConnection::<Db>::new().await.unwrap();
        let added_user = db
            .add_user(&DbUser::new("test_user", 2000))
            .await
            .unwrap()
            .unwrap();
        let added_wager = db
            .add_wager(&DbWager::new("test", "test", 200))
            .await
            .unwrap()
            .unwrap();
        let added_wager_option = db
            .add_wager_option_db(&DbWagerOption::new(
                "test wager option",
                "a test wager option",
                added_wager.id.clone(),
            ))
            .await
            .unwrap()
            .unwrap();

        let added_bet = db
            .add_bet_db(&DbBet::new(
                added_user.id,
                added_wager_option.id.clone(),
                200,
            ))
            .await
            .unwrap()
            .unwrap();
        let fetched_wager_option = db
            .select::<DbWagerOption>(&added_wager_option.id)
            .await
            .unwrap()
            .unwrap();
        assert!(fetched_wager_option.bets.contains(&added_bet.id));
    }

    struct DatabaseSetup {
        pub database_connection: DatabaseConnection<Db>,
        pub wagers: Vec<Thing>,
        pub wager_options: Vec<Thing>,
        pub bets: Vec<Thing>,
        pub users: Vec<Thing>,
    }

    impl DatabaseSetup {
        async fn new() -> Result<Self> {
            let database_connection = DatabaseConnection::<Db>::new().await?;

            let wagers: Vec<Thing> = vec![];
            let wager_options: Vec<Thing> = vec![];
            let bets: Vec<Thing> = vec![];
            let users: Vec<Thing> = vec![];

            Ok(Self {
                database_connection,
                wagers,
                bets,
                wager_options,
                users
            })
        }
    }

    async fn setup_testing_database() -> Result<DatabaseSetup> {
        let mut setup = DatabaseSetup::new().await?;

        let user1 = setup.database_connection.add_user(&DbUser::new("user1", 2000)).await?.unwrap().id;
        let user2 = setup.database_connection.add_user(&DbUser::new("user2", 2000)).await?.unwrap().id;
        setup.users.push(user1.clone());
        setup.users.push(user2.clone());
        
        let wager1 = setup.database_connection.add_wager(&DbWager::new("wager1", "wager1", 200)).await?.unwrap().id;
        let wager2 = setup.database_connection.add_wager(&DbWager::new("wager2", "wager2", 200)).await?.unwrap().id;
        setup.wagers.push(wager1.clone());
        setup.wagers.push(wager2.clone());
        
        let wager1_option1 = setup.database_connection.add_wager_option_db(&DbWagerOption::new("wager1_option1", "wager1_option1", wager1.clone())).await?.unwrap().id;
        let wager1_option2 = setup.database_connection.add_wager_option_db(&DbWagerOption::new("wager1_option2", "wager1_option2", wager1.clone())).await?.unwrap().id;
        setup.wager_options.push(wager1_option1.clone());
        setup.wager_options.push(wager1_option2.clone());
        
        let wager2_option1 = setup.database_connection.add_wager_option_db(&DbWagerOption::new("wager2_option1", "wager2_option1", wager2.clone())).await?.unwrap().id;
        let wager2_option2 = setup.database_connection.add_wager_option_db(&DbWagerOption::new("wager2_option2", "wager2_option2", wager2.clone())).await?.unwrap().id;
        setup.wager_options.push(wager2_option1.clone());
        setup.wager_options.push(wager2_option2.clone());
        
        let bet1 = setup.database_connection.add_bet_db(&DbBet::new(user1.clone(), wager1_option1.clone(), 200)).await?.unwrap().id;
        let bet2 = setup.database_connection.add_bet_db(&DbBet::new(user2.clone(), wager1_option2.clone(),200)).await?.unwrap().id;
        let bet3 = setup.database_connection.add_bet_db(&DbBet::new(user1.clone(), wager2_option1.clone(), 200)).await?.unwrap().id;
        let bet4 = setup.database_connection.add_bet_db(&DbBet::new(user2.clone(), wager2_option2.clone(), 200)).await?.unwrap().id;
        setup.bets.push(bet1);
        setup.bets.push(bet2);
        setup.bets.push(bet3);
        setup.bets.push(bet4);
        
        Ok(setup)
    }

    #[tokio::test]
    async fn test_remove_bet() {
        let mut setup = setup_testing_database().await.unwrap();
        let removed_bet = setup.bets.get(0).unwrap();

        setup.database_connection.remove_bet(&removed_bet).await.expect("should be able to remove bet");

        let fetched_user = setup.database_connection.select::<DbUser>(setup.users.get(0).clone().unwrap()).await.unwrap().expect("user should exist");
        assert_eq!(&fetched_user.balance, &2200);
        let fetched_wager_option = setup.database_connection.select::<DbWagerOption>(setup.wager_options.get(0).clone().unwrap()).await.unwrap().expect("wager option should exist");
        assert!(!fetched_wager_option.bets.contains(&removed_bet));

        let fetched_removed_bet = setup.database_connection.select::<DbBet>(removed_bet).await.unwrap();
        assert_eq!(fetched_removed_bet, None);
    }

    #[tokio::test]
    async fn test_remove_wager_option() {
        let mut setup = setup_testing_database().await.unwrap();
        let removed_wager_option = setup.wager_options.get(0).clone().unwrap();

        setup.database_connection.remove_wager_option(&removed_wager_option).await.unwrap();

        let fetched_user = setup.database_connection.select::<DbUser>(setup.users.get(0).clone().unwrap()).await.unwrap().expect("user should exist");
        assert_eq!(&fetched_user.balance, &2200);

        let fetched_wager = setup.database_connection.select::<DbWager>(setup.wagers.get(0).clone().unwrap()).await.unwrap().expect("wager should exist");
        assert!(!fetched_wager.options.contains(&removed_wager_option));

        let expected_removed_bet = setup.bets.get(0).unwrap();
        let fetched_removed_bet = setup.database_connection.select::<DbBet>(expected_removed_bet).await.unwrap();
        assert_eq!(fetched_removed_bet, None);

        let fetched_removed_wager_option = setup.database_connection.select::<DbWagerOption>(removed_wager_option).await.unwrap();
        assert_eq!(fetched_removed_wager_option, None);
    }

    #[tokio::test]
    async fn test_remove_wager() {
        let mut setup = setup_testing_database().await.unwrap();
        let removed_wager = setup.wagers.get(0).clone().unwrap();

        setup.database_connection.remove_wager(&removed_wager).await.unwrap();

        // user 1
        let fetched_user = setup.database_connection.select::<DbUser>(setup.users.get(0).clone().unwrap()).await.unwrap().expect("user should exist");
        assert_eq!(&fetched_user.balance, &2200);
        // user 2
        let fetched_user = setup.database_connection.select::<DbUser>(setup.users.get(1).clone().unwrap()).await.unwrap().expect("user should exist");
        assert_eq!(&fetched_user.balance, &2200);

        // option 1
        let fetched_wager_option = setup.database_connection.select::<DbWagerOption>(setup.wager_options.get(0).clone().unwrap()).await.unwrap();
        assert_eq!(fetched_wager_option, None);
        // option 2
        let fetched_wager_option = setup.database_connection.select::<DbWagerOption>(setup.wager_options.get(1).clone().unwrap()).await.unwrap();
        assert_eq!(fetched_wager_option, None);

        //bet 1
        let fetched_removed_bet = setup.database_connection.select::<DbBet>(setup.bets.get(0).unwrap()).await.unwrap();
        assert_eq!(fetched_removed_bet, None);
        // bet 2
        let fetched_removed_bet = setup.database_connection.select::<DbBet>(setup.bets.get(1).unwrap()).await.unwrap();
        assert_eq!(fetched_removed_bet, None);

        let fetched_removed_wager = setup.database_connection.select::<DbWager>(removed_wager).await.unwrap();
        assert_eq!(fetched_removed_wager, None);
    }

    #[tokio::test]
    async fn test_get_all_wagers() {
        let setup = setup_testing_database().await.unwrap();

        let all_wagers = vec!{
            DbWager {
                id: setup.wagers.get(1).clone().unwrap().to_owned(),
                name: "wager2".to_string(),
                description: "wager2".to_string(),
                options: vec![
                    setup.wager_options.get(2).clone().unwrap().to_owned(),
                    setup.wager_options.get(3).clone().unwrap().to_owned()
                ],
                pot: 200
            },
            DbWager {
                id: setup.wagers.get(0).clone().unwrap().to_owned(),
                name: "wager1".to_string(),
                description: "wager1".to_string(),
                options: vec![
                    setup.wager_options.get(0).clone().unwrap().to_owned(),
                    setup.wager_options.get(1).clone().unwrap().to_owned()
                ],
                pot: 200
            },
        };

        let all_fetched_wagers = setup.database_connection.get_all_wagers().await.expect("should be able to get all wagers");
        assert_eq!(all_fetched_wagers.len(), 2);
        for wager in &all_wagers {
            assert!(all_fetched_wagers.contains(wager));
        }
    }

    #[tokio::test]
    async fn test_get_all_bets_for_wager() {
        let mut setup = setup_testing_database().await.unwrap();

        let bet2 = setup.database_connection.add_bet_db(&DbBet::new(setup.users.get(1).cloned().unwrap(), setup.wager_options.get(0).cloned().unwrap(), 200)).await.unwrap().unwrap();

        let all_bets_for_option = vec![
            setup.database_connection.select::<DbBet>(setup.bets.get(0).unwrap()).await.unwrap().expect("should find bet"),
            setup.database_connection.select::<DbBet>(&bet2.id).await.unwrap().expect("should find bet"),
        ];

        let fetched_bets = setup.database_connection.get_all_bets_for_wager_option(&setup.wager_options.get(0).unwrap().id.to_string()).await.unwrap();
        assert_eq!(fetched_bets.len(), 2);
        for bet in &all_bets_for_option {
            assert!(fetched_bets.contains(bet));
        }
    }

    #[tokio::test]
    async fn test_get_all_bets_for_user() {
        let mut setup = setup_testing_database().await.unwrap();

        let all_bets_for_user = vec![
            setup.database_connection.select::<DbBet>(setup.bets.get(0).unwrap()).await.unwrap().unwrap(),
            setup.database_connection.select::<DbBet>(setup.bets.get(2).unwrap()).await.unwrap().unwrap(),
        ];

        let fetched_bets = setup.database_connection.get_bets_by_user("user1").await.unwrap();
        assert_eq!(fetched_bets.len(), all_bets_for_user.len());
        for bet in &all_bets_for_user {
            assert!(fetched_bets.contains(bet));
        }
    }
}
