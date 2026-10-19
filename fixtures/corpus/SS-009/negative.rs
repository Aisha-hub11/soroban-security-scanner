#[contractimpl]
impl Auction {
    pub fn close(env: Env) {
        // The timestamp is only compared with a stored deadline, not used as
        // randomness.
        let deadline: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Deadline)
            .unwrap();
        if env.ledger().timestamp() > deadline {
            env.storage().persistent().set(&DataKey::Closed, &true);
        }
    }
}
