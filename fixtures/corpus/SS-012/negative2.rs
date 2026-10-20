// SS-012 negative: capped collection and a local collection.

#![no_std]

use soroban_sdk::{contractimpl, Env, Vec};

pub struct Registry;

#[contractimpl]
impl Registry {
    pub fn add(env: Env, item: u32) {
        let mut items: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::Items)
            .unwrap_or(Vec::new(&env));
        if items.len() >= 100 {
            panic!("cap reached");
        }
        items.push_back(item);
        env.storage().persistent().set(&DataKey::Items, &items);
    }

    pub fn local(env: Env) -> Vec<u32> {
        let mut items = Vec::new(&env);
        items.push_back(1);
        items
    }
}
