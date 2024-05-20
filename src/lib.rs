use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use surrealdb::opt::auth::Root;
use surrealdb::sql::Thing;
use surrealdb::Surreal;
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    Result,
};

const BUFFER_LIMIT: usize = 1048576;

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum Request {
    Login { user: String },
    WhoAmI,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Response {
    Ok(RequestResponse),
    Error,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum RequestResponse {
    None,
    WhoAmI(String),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Packet {
    RequestPacket(Request),
    ResponsePacket(Response),
}

pub struct Connection {
    connection: TcpStream,
    buf: Vec<u8>,
    cursor: usize,
}

impl Connection {
    pub fn from_tcp_stream(connection: TcpStream) -> Self {
        Self {
            connection,
            buf: vec![0; 4096],
            cursor: 0,
        }
    }

    pub async fn read(&mut self) -> tokio::io::Result<Packet> {
        let mut last_error = tokio::io::Error::new(ErrorKind::Other, "");
        let mut parse_cursor = self.cursor;
        loop {
            while parse_cursor <= self.cursor {
                let res = rmp_serde::from_slice(&self.buf[..parse_cursor]);
                match res {
                    Ok(req) => {
                        self.buf.drain(..parse_cursor);
                        self.cursor = parse_cursor - self.cursor;
                        return tokio::io::Result::Ok(req);
                    }
                    Err(_) => {
                        last_error =
                            tokio::io::Error::new(ErrorKind::InvalidData, "malformed request")
                    }
                }
                parse_cursor += 1;
            }

            self.grow_buffer_if_needed();
            if self.buf.len() >= BUFFER_LIMIT {
                return Err(last_error);
            }
            let n = self.connection.read(&mut self.buf[self.cursor..]).await?;
            if n == 0 {
                let e = tokio::io::Error::new(ErrorKind::ConnectionAborted, "connection closed");
                return Err(e);
            }
            self.cursor += n;
        }
    }

    pub async fn send(&mut self, data: Packet) -> tokio::io::Result<()> {
        self.connection
            .write_all(&rmp_serde::to_vec(&data).unwrap())
            .await
    }

    fn grow_buffer_if_needed(&mut self) {
        if self.buf.len() == self.cursor {
            self.buf.resize(self.buf.len() * 2, 0);
        }
    }
}

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
        let mut res: Vec<Record> = self.connection.create("user").content(user).await?;

        Ok(res.pop().unwrap()) //there should be one (and only one) element created
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
}
