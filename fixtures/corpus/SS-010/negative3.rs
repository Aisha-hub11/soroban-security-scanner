// SS-010 negative: privileged write publishes an event.

#![no_std]

use soroban_sdk::{contractimpl, symbol_short, Address, Env};

pub struct Config;

#[contractimpl]
impl Config {
    pub fn set_admin(env: Env, admin: Address) {
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.events().publish((symbol_short!("admin"),), admin);
    }
}
