use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[contract]
pub struct ContractTypes;

/// Stored contract configuration.
#[contracttype]
pub struct Config {
    pub admin: Address,
    pub enabled: bool,
}

/// Storage keys used by the contract.
#[contracttype]
pub enum DataKey {
    Admin,
    Config,
    Balance(Address),
}

/// Contract error variants.
#[contracterror]
pub enum VaultError {
    NotInitialized = 1,
    Unauthorized = 2,
}

#[contractimpl]
impl ContractTypes {
    pub fn admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }
}
