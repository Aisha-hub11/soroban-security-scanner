// SS-016 negative: one instance write and persistent storage.

#![no_std]

use soroban_sdk::{contractimpl, Env};

pub struct Settings;

#[contractimpl]
impl Settings {
    pub fn configure(env: Env, a: u32, b: u32) {
        env.storage().instance().set(&DataKey::A, &a);
        env.storage().persistent().set(&DataKey::B, &b);
    }
}
