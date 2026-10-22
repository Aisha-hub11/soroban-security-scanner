// SS-016 positive: aliased instance-storage calls.
// This file is a security fixture, not production code.

#![no_std]

use soroban_sdk::{contractimpl, storage::Instance as Store, Env};

pub struct Settings;

#[contractimpl]
impl Settings {
    pub fn configure(env: Env, a: u32, b: u32) {
        let store = env.storage().instance();
        Store::set(&store, &DataKey::A, &a);
        Store::set(&store, &DataKey::B, &b);
    }
}
