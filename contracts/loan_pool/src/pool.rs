use crate::{
    error::LoanPoolError,
    storage_types::{extend_persistent, PoolDataKey},
};
use soroban_sdk::{contracttype, Address, Env, Symbol};

#[contracttype]
pub struct Currency {
    pub token_address: Address,
    pub ticker: Symbol,
}

pub fn write_loan_manager_addr(e: &Env, loan_manager_addr: Address) {
    let key = PoolDataKey::LoanManagerAddress;

    e.storage().persistent().set(&key, &loan_manager_addr);
    extend_persistent(e.clone(), &key);
}

pub fn read_loan_manager_addr(e: &Env) -> Result<Address, LoanPoolError> {
    let key = PoolDataKey::LoanManagerAddress;

    if let Some(loan_manager_address) = e.storage().persistent().get(&key) {
        loan_manager_address
    } else {
        Err(LoanPoolError::LoanManager)
    }
}

pub fn write_currency(e: &Env, currency: Currency) {
    let key = PoolDataKey::Currency;

    e.storage().persistent().set(&key, &currency);
    extend_persistent(e.clone(), &key);
}

pub fn read_currency(e: &Env) -> Result<Currency, LoanPoolError> {
    let key = PoolDataKey::Currency;

    if let Some(currency) = e.storage().persistent().get(&key) {
        currency
    } else {
        Err(LoanPoolError::Currency)
    }
}

pub fn write_liquidation_threshold(e: &Env, threshold: i128) {
    let key = PoolDataKey::LiquidationThreshold;

    e.storage().persistent().set(&key, &threshold);
    extend_persistent(e.clone(), &key);
}

pub fn write_total_shares(e: &Env, amount: i128) {
    let key: PoolDataKey = PoolDataKey::TotalBalanceShares;

    e.storage().persistent().set(&key, &amount);
    extend_persistent(e.clone(), &key);
}

pub fn read_total_shares(e: &Env) -> Result<i128, LoanPoolError> {
    let key: PoolDataKey = PoolDataKey::TotalBalanceShares;

    if let Some(total_shares) = e.storage().persistent().get(&key) {
        total_shares
    } else {
        Err(LoanPoolError::TotalShares)
    }
}

pub fn change_total_shares(e: &Env, amount: i128) -> Result<i128, LoanPoolError> {
    let current_balance = read_total_shares(e)?;

    let new_amount = amount
        .checked_add(current_balance)
        .ok_or(LoanPoolError::OverOrUnderFlow)?;
    write_total_shares(e, new_amount);
    Ok(new_amount)
}

pub fn write_total_balance(e: &Env, amount: i128) {
    let key: PoolDataKey = PoolDataKey::TotalBalanceTokens;

    e.storage().persistent().set(&key, &amount);
    extend_persistent(e.clone(), &key);
}

pub fn read_total_balance(e: &Env) -> Result<i128, LoanPoolError> {
    let key: PoolDataKey = PoolDataKey::TotalBalanceTokens;

    if let Some(total_balance) = e.storage().persistent().get(&key) {
        total_balance
    } else {
        Err(LoanPoolError::TotalBalance)
    }
}

pub fn change_total_balance(e: &Env, amount: i128) -> Result<i128, LoanPoolError> {
    let current_balance = read_total_balance(e)?;

    let new_amount = amount
        .checked_add(current_balance)
        .ok_or(LoanPoolError::OverOrUnderFlow)?;
    write_total_balance(e, new_amount);
    Ok(new_amount)
}

pub fn write_available_balance(e: &Env, amount: i128) {
    let key: PoolDataKey = PoolDataKey::AvailableBalanceTokens;

    e.storage().persistent().set(&key, &amount);
    extend_persistent(e.clone(), &key);
}

pub fn read_available_balance(e: &Env) -> Result<i128, LoanPoolError> {
    let key: PoolDataKey = PoolDataKey::AvailableBalanceTokens;

    if let Some(available_balance) = e.storage().persistent().get(&key) {
        available_balance
    } else {
        Err(LoanPoolError::AvailableBalance)
    }
}

pub fn change_available_balance(e: &Env, amount: i128) -> Result<i128, LoanPoolError> {
    let current_balance = read_available_balance(e)?;

    let new_amount = amount
        .checked_add(current_balance)
        .ok_or(LoanPoolError::OverOrUnderFlow)?;
    write_available_balance(e, new_amount);
    Ok(new_amount)
}

pub fn write_accrual(e: &Env, accrual: i128) {
    let key = PoolDataKey::Accrual;

    e.storage().persistent().set(&key, &accrual);
    extend_persistent(e.clone(), &key);
}

pub fn read_accrual(e: &Env) -> Result<i128, LoanPoolError> {
    let key = PoolDataKey::Accrual;

    if let Some(accrual) = e.storage().persistent().get(&key) {
        accrual
    } else {
        Err(LoanPoolError::Accrual)
    }
}

pub fn write_accrual_last_updated(e: &Env, sequence: u64) -> u64 {
    let key = PoolDataKey::AccrualLastUpdate;

    e.storage().persistent().set(&key, &sequence);
    extend_persistent(e.clone(), &key);

    sequence
}

pub fn read_accrual_last_updated(e: &Env) -> Result<u64, LoanPoolError> {
    let key = PoolDataKey::AccrualLastUpdate;

    if let Some(accrual_last_updated) = e.storage().persistent().get(&key) {
        accrual_last_updated
    } else {
        Err(LoanPoolError::AccrualLastUpdated)
    }
}

pub fn change_interest_rate_multiplier(e: &Env, multiplier: i128) {
    e.storage()
        .persistent()
        .set(&PoolDataKey::InterestRateMultiplier, &multiplier);
}

pub fn read_interest_rate_multiplier(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::InterestRateMultiplier)
        .ok_or(LoanPoolError::InterestRateMultiplier)
}

pub fn read_collateral_factor(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::LiquidationThreshold)
        .ok_or(LoanPoolError::LiquidationThreshold)
}
