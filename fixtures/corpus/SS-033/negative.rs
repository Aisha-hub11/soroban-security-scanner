// SS-033 negative: explicit Soroban SDK imports.
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn ping(env: Env) -> Symbol {
        symbol_short!("pong")
    }
}
