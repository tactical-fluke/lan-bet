use serde::{Deserialize, Serialize};

pub mod network;

pub struct User {
    pub name: String,
    pub balance: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Wager {
    pub id: String,
    pub name: String,
    pub description: String,
    pub pot: u64,
    pub options: Vec<WagerOption>,
}

// A potential, unrealised wager. To be used the creation of a new, fully realised wager
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct PotentialWager {
    pub name: String,
    pub description: String,
    pub pot: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct WagerOption {
    pub id: String,
    pub name: String,
    pub description: String,
    pub bets: Vec<Bet>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Bet {
    pub id: String,
    pub user_id: String,
    pub val: u64,
}
