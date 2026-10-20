// SS-017 negative: authorization precedes the mutation.

#![no_std]

use soroban_sdk::{contractimpl, Address, Env};

pub struct Vault;

#[contractimpl]
impl Vault {
    pub fn set(env: Env, admin: Address, value: u32) {
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Value, &value);
    }
}
