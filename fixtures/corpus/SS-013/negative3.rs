#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, to: Address, amount: i128) {
        // Effects first: update state before the cross-contract call.
        env.storage().persistent().set(&Key::Pending, &amount);
        let client = TokenClient::new(&env, &to);
        client.transfer(&env.current_contract_address(), &to, &amount);
    }
}
