use soroban_sdk::{contracttype, Address, Env};

use crate::error::InsurancePoolError;

#[derive(Clone)]
#[contracttype]
pub struct InsurancePositions {
    pub insurance_shares: i128,
}

#[derive(Clone)]
#[contracttype]
enum InsurancePoolDataKey {
    // The paired loan pool address (authorized to call cover_bad_debt)
    LoanPoolAddress,
    // The share token (lXLM/lUSDC/lEURC) held by this insurance pool
    ShareTokenAddress,
    // Per-depositor internal insurance shares
    InsurancePositions(Address),
    // Total insurance shares issued across all depositors
    TotalInsuranceShares,
    // Total share tokens (lXLM etc) held by this contract
    TotalInsuranceTokens,
}

const DAY_IN_LEDGERS: u32 = 17280;
const POSITIONS_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const POSITIONS_LIFETIME_THRESHOLD: u32 = POSITIONS_BUMP_AMOUNT - DAY_IN_LEDGERS;

fn extend_persistent(e: &Env, key: &InsurancePoolDataKey) {
    e.storage()
        .persistent()
        .extend_ttl(key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
}

pub fn write_loan_pool_address(e: &Env, address: Address) {
    let key = InsurancePoolDataKey::LoanPoolAddress;
    e.storage().persistent().set(&key, &address);
    extend_persistent(e, &key);
}

pub fn read_loan_pool_address(e: &Env) -> Result<Address, InsurancePoolError> {
    e.storage()
        .persistent()
        .get(&InsurancePoolDataKey::LoanPoolAddress)
        .ok_or(InsurancePoolError::LoanPoolAddress)
}

pub fn write_share_token_address(e: &Env, address: Address) {
    let key = InsurancePoolDataKey::ShareTokenAddress;
    e.storage().persistent().set(&key, &address);
    extend_persistent(e, &key);
}

pub fn read_share_token_address(e: &Env) -> Result<Address, InsurancePoolError> {
    e.storage()
        .persistent()
        .get(&InsurancePoolDataKey::ShareTokenAddress)
        .ok_or(InsurancePoolError::ShareTokenAddress)
}

pub fn read_insurance_positions(e: &Env, addr: &Address) -> InsurancePositions {
    let key = InsurancePoolDataKey::InsurancePositions(addr.clone());
    if let Some(positions) = e.storage().persistent().get(&key) {
        extend_persistent(e, &key);
        positions
    } else {
        InsurancePositions {
            insurance_shares: 0,
        }
    }
}

pub fn write_insurance_positions(e: &Env, addr: Address, insurance_shares: i128) {
    let key = InsurancePoolDataKey::InsurancePositions(addr);
    let positions = InsurancePositions { insurance_shares };
    e.storage().persistent().set(&key, &positions);
    extend_persistent(e, &key);
}

pub fn read_total_insurance_shares(e: &Env) -> i128 {
    e.storage()
        .persistent()
        .get(&InsurancePoolDataKey::TotalInsuranceShares)
        .unwrap_or(0)
}

pub fn write_total_insurance_shares(e: &Env, amount: i128) {
    let key = InsurancePoolDataKey::TotalInsuranceShares;
    e.storage().persistent().set(&key, &amount);
    extend_persistent(e, &key);
}

pub fn read_total_insurance_tokens(e: &Env) -> i128 {
    e.storage()
        .persistent()
        .get(&InsurancePoolDataKey::TotalInsuranceTokens)
        .unwrap_or(0)
}

pub fn write_total_insurance_tokens(e: &Env, amount: i128) {
    let key = InsurancePoolDataKey::TotalInsuranceTokens;
    e.storage().persistent().set(&key, &amount);
    extend_persistent(e, &key);
}
