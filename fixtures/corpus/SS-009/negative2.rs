#[contractimpl]
impl Clock {
    pub fn now(env: Env) -> u64 {
        // Reading the ledger time is not by itself insecure randomness.
        env.ledger().timestamp()
    }
}
