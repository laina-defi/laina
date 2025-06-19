use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

use crate::error::LoanPoolError;

/* Storage Types */

// Config for pool
#[derive(Clone)]
#[contracttype]
pub struct PoolConfig {
    pub oracle: Address, // The contract address for the price oracle
    pub status: u32,     // Status of the pool
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
#[contracttype]
pub struct Positions {
    // struct names under 9 characters are marginally more efficient. Need to think if we value marginal efficiency over readibility
    pub receivable_shares: i128,
    pub liabilities: i128,
    pub collateral: i128,
}

#[contracttype]
pub struct Currency {
    pub token_address: Address,
    pub ticker: Symbol,
}

#[derive(Clone)]
#[contracttype]
enum PoolDataKey {
    // Address of the loan manager for authorization.
    LoanManagerAddress,
    // Pool's token's address & ticker
    Currency,
    // The threshold when a loan should liquidate, unit is one-millionth
    LiquidationThreshold,
    // Users positions in the pool
    Positions(Address),
    // Total amount of shares in circulation
    TotalBalanceShares,
    // Total balance of pool
    TotalBalanceTokens,
    // Available balance of pool
    AvailableBalanceTokens,
    // Pool interest accrual index
    Accrual,
    // Last update ledger of accrual
    AccrualLastUpdate,
    // Interest rate multiplier
    InterestRateMultiplier,
}

/* Ledger Thresholds */

pub(crate) const DAY_IN_LEDGERS: u32 = 17280; // if ledger takes 5 seconds

pub(crate) const POSITIONS_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const POSITIONS_LIFETIME_THRESHOLD: u32 = POSITIONS_BUMP_AMOUNT - DAY_IN_LEDGERS;

/* Persistent ttl bumper */
fn extend_persistent(e: &Env, key: &PoolDataKey) {
    e.storage()
        .persistent()
        .extend_ttl(key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
}

pub fn write_loan_manager_addr(e: &Env, loan_manager_addr: Address) {
    let key = PoolDataKey::LoanManagerAddress;
    e.storage().persistent().set(&key, &loan_manager_addr);
    extend_persistent(e, &key);
    e.events()
        .publish((key, symbol_short!("added")), loan_manager_addr);
}

pub fn read_loan_manager_addr(e: &Env) -> Result<Address, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::LoanManagerAddress)
        .ok_or(LoanPoolError::LoanManager)
}

pub fn write_currency(e: &Env, currency: Currency) {
    let key = PoolDataKey::Currency;
    e.storage().persistent().set(&key, &currency);
    extend_persistent(e, &key);
    e.events().publish((key, symbol_short!("added")), currency);
}

pub fn read_currency(e: &Env) -> Result<Currency, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::Currency)
        .ok_or(LoanPoolError::Currency)
}

pub fn write_liquidation_threshold(e: &Env, threshold: i128) {
    let key = PoolDataKey::LiquidationThreshold;
    e.storage().persistent().set(&key, &threshold);
    extend_persistent(e, &key);
    e.events().publish((key, symbol_short!("added")), threshold);
}

pub fn write_total_shares(e: &Env, amount: i128) {
    let key = PoolDataKey::TotalBalanceShares;
    e.storage().persistent().set(&key, &amount);
    extend_persistent(e, &key);
    e.events().publish((key, symbol_short!("updated")), amount);
}

pub fn read_total_shares(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::TotalBalanceShares)
        .ok_or(LoanPoolError::TotalShares)
}

pub fn adjust_total_shares(e: &Env, amount: i128) -> Result<i128, LoanPoolError> {
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
    extend_persistent(e, &key);
    e.events().publish((key, symbol_short!("added")), amount);
}

pub fn read_total_balance(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::TotalBalanceTokens)
        .ok_or(LoanPoolError::TotalBalance)
}

pub fn adjust_total_balance(e: &Env, amount: i128) -> Result<i128, LoanPoolError> {
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
    extend_persistent(e, &key);
    e.events().publish((key, "added"), amount);
}

pub fn read_available_balance(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::AvailableBalanceTokens)
        .ok_or(LoanPoolError::AvailableBalance)
}

pub fn adjust_available_balance(e: &Env, amount: i128) -> Result<i128, LoanPoolError> {
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
    extend_persistent(e, &key);
    e.events().publish((key, "updated"), accrual);
}

pub fn read_accrual(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::Accrual)
        .ok_or(LoanPoolError::Accrual)
}

pub fn write_accrual_last_updated(e: &Env, sequence: u64) -> u64 {
    let key = PoolDataKey::AccrualLastUpdate;

    e.storage().persistent().set(&key, &sequence);
    extend_persistent(e, &key);
    e.events().publish((key, "updated"), e.ledger().timestamp());

    sequence
}

pub fn read_accrual_last_updated(e: &Env) -> Result<u64, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::AccrualLastUpdate)
        .ok_or(LoanPoolError::AccrualLastUpdated)
}

pub fn change_interest_rate_multiplier(e: &Env, multiplier: i128) {
    let key = PoolDataKey::InterestRateMultiplier;
    e.storage().persistent().set(&key, &multiplier);
    e.events()
        .publish((key, symbol_short!("updated")), multiplier)
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

pub fn read_positions(e: &Env, addr: &Address) -> Positions {
    let key = PoolDataKey::Positions(addr.clone());
    if let Some(positions) = e.storage().persistent().get(&key) {
        extend_persistent(e, &key);
        positions
    } else {
        Positions {
            receivable_shares: 0,
            liabilities: 0,
            collateral: 0,
        }
    }
}

pub fn write_positions(
    e: &Env,
    addr: Address,
    receivables: i128,
    liabilities: i128,
    collateral: i128,
) {
    let key = PoolDataKey::Positions(addr.clone());

    let positions = Positions {
        receivable_shares: receivables,
        liabilities,
        collateral,
    };
    e.storage().persistent().set(&key, &positions);
    extend_persistent(e, &key);

    e.events()
        .publish((symbol_short!("positions"), symbol_short!("updated")), addr)
}
