// SS-033 negative: a glob import from an unrelated crate is not an SDK import.
use std::collections::*;

use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn ping(env: Env) -> Symbol {
        let _values: BTreeMap<u32, u32> = BTreeMap::new();
        symbol_short!("pong")
    }
}
