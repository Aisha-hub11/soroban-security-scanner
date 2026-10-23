// SS-033 negative: an explicit nested module import.
use soroban_sdk::token::TokenClient;
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn ping(_env: Env) {}

    pub fn client(env: Env) -> TokenClient {
        TokenClient::new(&env, &env.current_contract_address())
    }
}
