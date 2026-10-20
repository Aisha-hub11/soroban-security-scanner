// SS-002 negative: the client address is trusted configuration.

#![no_std]

use soroban_sdk::{contractimpl, Env};

pub struct Payments;

#[contractimpl]
impl Payments {
    pub fn pay(env: Env, amount: i128) {
        let client = TokenClient::new(
            &env,
            "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM",
        );
        client.transfer(&env.current_contract_address(), &amount);
    }

    pub fn pay_configured(env: Env, amount: i128) {
        let token = env.storage().persistent().get(&DataKey::Token).unwrap();
        let client = TokenClient::new(&env, &token);
        client.transfer(&env.current_contract_address(), &amount);
    }
}
