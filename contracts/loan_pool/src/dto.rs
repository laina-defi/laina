use soroban_sdk::contracttype;

#[contracttype]
#[derive(Debug, PartialEq)]
pub struct PoolState {
    pub total_balance_tokens: i128,
    pub available_balance_tokens: i128,
    pub total_balance_shares: i128,
    pub annual_interest_rate: i128,
}
