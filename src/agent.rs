use crate::transaction::Holding;

use rand::{random, Rng};
use serde::{Deserialize, Serialize};

static MAX_INITIAL_BALANCE: f64 = 1000.0;

#[derive(Serialize, Deserialize, Debug)]
pub struct Agent {
    id: u64,
    /// How much money does the agent have
    balance: f64,
    /// How many shares does an agent hold in a company
    holdings: Vec<Holding>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Company {
    id: u64,
    name: String,
    code: [char; 3],
    /// Number of total shares
    total_shares: u64,
}

fn random_string() -> String {
    return "lmao".to_string();
}

fn rand_char() -> char {
    let mut rng = rand::thread_rng();
    let mut i: u8 = rng.gen_range(0..52);
    if i < 26 {
        return ('a' as u8 + i) as char;
    }
    i -= 26;
    return ('A' as u8 + i) as char;
}

impl Agent {
    pub fn new(id: u64, balance: f64, holdings: Vec<Holding>) -> Self {
        Self {
            id,
            balance,
            holdings,
        }
    }
    pub fn rand() -> Self {
        Self::new(random(), random::<f64>() * MAX_INITIAL_BALANCE, Vec::new())
    }
}

impl Company {
    pub fn new(id: u64, name: String, code: [char; 3], total_shares: u64) -> Self {
        Self {
            id,
            name,
            code,
            total_shares,
        }
    }
    pub fn rand() -> Self {
        Self::new(
            random(),
            random_string(),
            [rand_char(), rand_char(), rand_char()],
            random(),
        )
    }
}
