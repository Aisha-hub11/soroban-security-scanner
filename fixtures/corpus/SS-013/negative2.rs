#[contractimpl]
impl Vault {
    pub fn balance(env: Env, id: Address) -> i128 {
        // No state is updated, so there is no ordering violation to report.
        let client = TokenClient::new(&env, &id);
        client.balance(&id)
    }
}
