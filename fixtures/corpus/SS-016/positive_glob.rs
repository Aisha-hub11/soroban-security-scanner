// SS-016 positive: glob-imported instance-storage calls.
// This file is a security fixture, not production code.

#![no_std]

use soroban_sdk::{contractimpl, storage::*, Env};

pub struct Settings;

#[contractimpl]
impl Settings {
    pub fn configure(env: Env, a: u32, b: u32) {
        let store = env.storage().instance();
        Instance::set(&store, &DataKey::A, &a);
        Instance::set(&store, &DataKey::B, &b);
    }
}
