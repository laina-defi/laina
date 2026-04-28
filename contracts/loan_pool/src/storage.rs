use soroban_sdk::{contractevent, contracttype, Address, Env, Symbol};

use crate::error::LoanPoolError;

/* Storage Types */

pub const DEFAULT_BASE_INTEREST_RATE: i128 = 200_000; // 2%
pub const DEFAULT_INTEREST_RATE_AT_PANIC: i128 = 1_000_000; // 10%
pub const DEFAULT_MAX_INTEREST_RATE: i128 = 3_000_000; // 30%
pub const DEFAULT_PANIC_RATES_THRESHOLD: i128 = 90_000_000;

pub const DECIMAL: i128 = 10_000_000;

// Config for pool
#[derive(Clone)]
#[contracttype]
pub struct PoolConfig {
    pub oracle: Address, // The contract address for the price oracle
    pub status: u32,     // Status of the pool
}

#[derive(Clone)]
#[contracttype]
pub struct Positions {
    // struct names under 9 characters are marginally more efficient. Need to think if we value marginal efficiency over readibility
    pub receivable_shares: i128,
    pub liabilities: i128,
    pub collateral_shares: i128,
}

/// Internal storage struct — receivable_shares are stored in the share token, not here.
#[derive(Clone)]
#[contracttype]
pub struct StoredPositions {
    pub liabilities: i128,
    pub collateral_shares: i128,
}

#[contracttype]
pub struct Currency {
    pub token_address: Address,
    pub ticker: Symbol,
}

#[derive(PartialEq, Eq, Debug)]
#[contracttype]
pub enum PoolStatus {
    Healthy,
    Caution,
    Restricted,
    Frozen,
}

#[contracttype]
pub struct InterestDto {
    pub interest_multiplier: i128,
    pub base_interest_rate: i128,
    pub interest_rate_at_panic: i128,
    pub max_interest_rate: i128,
    pub panic_rates_threshold: i128,
    pub collateral_factor: i128,
}

#[derive(Clone)]
#[contracttype]
enum PoolDataKey {
    // Address of the loan manager for authorization.
    LoanManagerAddress,
    // Pool's token's address & ticker
    Currency,
    // The threshold when a loan should liquidate, unit is one-millionth
    CollateralFactor,
    PanicRatesThreshold,
    MaxInterestRate,
    InterestRateAtPanic,
    BaseInterestRate,
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
    // Pool health status,
    PoolStatus,
    // Token contract address for share token,
    TokenContractAddress,
    // Insurance pool contract address,
    InsurancePoolAddress,
    // Total outstanding borrower liabilities including accrued interest.
    // Mirrors the sum of all per-user positions.liabilities.  Kept separately
    // from total_balance so the LAI accumulator denominator stays correct as
    // interest compounds via increase_liabilities.
    TotalLiabilitiesTokens,
}

/* Contract events */
#[contractevent(topics = ["pool_status_updated"])]
pub struct EventPoolStatusUpdated {
    pub pool_status: PoolStatus,
}

#[contractevent(topics = ["token_contract_address_added"])]
pub struct EventTokenContractAddressAdded {
    pub token_contract_address: Address,
}

#[contractevent(topics = ["loan_manager_address_added"])]
pub struct EventLoanManagerAddressAdded {
    pub loan_manager_addr: Address,
}

#[contractevent(topics = ["currency_added"])]
pub struct EventCurrencyAdded {
    pub currency: Currency,
}

#[contractevent(topics = ["collateral_factor_changed"])]
pub struct EventCollateralFactorChanged {
    pub collateral_factor: i128,
}

#[contractevent(topics = ["total_shares_updated"])]
pub struct EventTotalSharesUpdated {
    pub amount: i128,
}

#[contractevent(topics = ["total_balance_changed"])]
pub struct EventTotalBalanceChanged {
    pub amount: i128,
}

#[contractevent(topics = ["available_balance_changed"])]
pub struct EventAvailableBalanceChanged {
    pub amount: i128,
}

#[contractevent(topics = ["accrual_changed"])]
pub struct EventAccrualChanged {
    pub accrual: i128,
}

#[contractevent(topics = ["accrual_last_updated"])]
pub struct EventAccrualLastUpdated {
    pub timestamp: u64,
}

#[contractevent(topics = ["interest_rate_multiplier_changed"])]
pub struct EventInterestMultiplierChanged {
    pub multiplier: i128,
}

#[contractevent(topics = ["base_interest_rate_changed"])]
pub struct EventBaseInterestRateChanged {
    pub base_interest_rate: i128,
}

#[contractevent(topics = ["interest_rate_at_panic_changed"])]
pub struct EventInterestRateAtPanicChanged {
    pub interest_rate_at_panic: i128,
}

#[contractevent(topics = ["max_interest_rate_changed"])]
pub struct EventMaxInterestRateChanged {
    pub max_interest_rate: i128,
}

#[contractevent(topics = ["panic_rates_threshold_changed"])]
pub struct EventPanicRatesThresholdChanged {
    pub panic_rates_threshold: i128,
}

#[contractevent(topics = ["positions_updated"])]
pub struct EventPositionsUpdated {
    pub addr: Address,
    pub positions: Positions,
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

pub fn change_pool_status(e: &Env, pool_status: PoolStatus) {
    let key = PoolDataKey::PoolStatus;
    e.storage().persistent().set(&key, &pool_status);
    extend_persistent(e, &key);
    EventPoolStatusUpdated { pool_status }.publish(e);
}

pub fn read_pool_status(e: &Env) -> Result<PoolStatus, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::PoolStatus)
        .ok_or(LoanPoolError::PoolStatus)
}

pub fn write_loan_manager_addr(e: &Env, loan_manager_addr: Address) {
    let key = PoolDataKey::LoanManagerAddress;
    e.storage().persistent().set(&key, &loan_manager_addr);
    extend_persistent(e, &key);
    EventLoanManagerAddressAdded {
        loan_manager_addr: loan_manager_addr.clone(),
    }
    .publish(e);
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
    EventCurrencyAdded { currency }.publish(e);
}

pub fn read_currency(e: &Env) -> Result<Currency, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::Currency)
        .ok_or(LoanPoolError::Currency)
}

pub fn write_collateral_factor(e: &Env, collateral_factor: i128) {
    let key = PoolDataKey::CollateralFactor;
    e.storage().persistent().set(&key, &collateral_factor);
    extend_persistent(e, &key);
    EventCollateralFactorChanged { collateral_factor }.publish(e);
}

pub fn write_total_shares(e: &Env, amount: i128) {
    let key = PoolDataKey::TotalBalanceShares;
    e.storage().persistent().set(&key, &amount);
    extend_persistent(e, &key);
    EventTotalSharesUpdated { amount }.publish(e);
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
    EventTotalBalanceChanged { amount }.publish(e);
}

pub fn read_total_balance(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::TotalBalanceTokens)
        .ok_or(LoanPoolError::TotalBalance)
}

pub fn write_token_contract_address(e: &Env, address: Address) {
    let key: PoolDataKey = PoolDataKey::TokenContractAddress;
    e.storage().instance().set(&key, &address);
    EventTokenContractAddressAdded {
        token_contract_address: address,
    }
    .publish(e);
}

pub fn read_token_contract_address(e: &Env) -> Result<Address, LoanPoolError> {
    e.storage()
        .instance()
        .get(&PoolDataKey::TokenContractAddress)
        .ok_or(LoanPoolError::TokenContractAddress)
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
    EventAvailableBalanceChanged { amount }.publish(e);
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
    EventAccrualChanged { accrual }.publish(e);
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
    EventAccrualLastUpdated {
        timestamp: e.ledger().timestamp(),
    }
    .publish(e);

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
    EventInterestMultiplierChanged { multiplier }.publish(e)
}

pub fn read_interest_rate_multiplier(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::InterestRateMultiplier)
        .ok_or(LoanPoolError::InterestRateMultiplier)
}

pub fn write_base_interest_rate(e: &Env, rate: i128) {
    e.storage()
        .instance()
        .set(&PoolDataKey::BaseInterestRate, &rate);
    EventBaseInterestRateChanged {
        base_interest_rate: rate,
    }
    .publish(e);
}

pub fn read_base_interest_rate(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .instance()
        .get(&PoolDataKey::BaseInterestRate)
        .ok_or(LoanPoolError::BaseInterestRate)
}

pub fn write_interest_rate_at_panic(e: &Env, rate: i128) {
    e.storage()
        .instance()
        .set(&PoolDataKey::InterestRateAtPanic, &rate);
    EventInterestRateAtPanicChanged {
        interest_rate_at_panic: rate,
    }
    .publish(e);
}

pub fn read_interest_rate_at_panic(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .instance()
        .get(&PoolDataKey::InterestRateAtPanic)
        .ok_or(LoanPoolError::InterestRateAtPanic)
}

pub fn write_max_interest_rate(e: &Env, rate: i128) {
    e.storage()
        .instance()
        .set(&PoolDataKey::MaxInterestRate, &rate);
    EventMaxInterestRateChanged {
        max_interest_rate: rate,
    }
    .publish(e);
}

pub fn read_max_interest_rate(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .instance()
        .get(&PoolDataKey::MaxInterestRate)
        .ok_or(LoanPoolError::MaxInterestRate)
}

pub fn write_panic_rates_threshold(e: &Env, threshold: i128) {
    e.storage()
        .instance()
        .set(&PoolDataKey::PanicRatesThreshold, &threshold);
    EventPanicRatesThresholdChanged {
        panic_rates_threshold: threshold,
    }
    .publish(e);
}

pub fn read_panic_rates_threshold(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .instance()
        .get(&PoolDataKey::PanicRatesThreshold)
        .ok_or(LoanPoolError::PanicRatesThreshold)
}

pub fn read_interest_dto(e: &Env) -> Result<InterestDto, LoanPoolError> {
    Ok(InterestDto {
        interest_multiplier: read_interest_rate_multiplier(e)?,
        base_interest_rate: read_base_interest_rate(e)?,
        interest_rate_at_panic: read_interest_rate_at_panic(e)?,
        max_interest_rate: read_max_interest_rate(e)?,
        panic_rates_threshold: read_panic_rates_threshold(e)?,
        collateral_factor: read_collateral_factor(e)?,
    })
}

pub fn read_collateral_factor(e: &Env) -> Result<i128, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::CollateralFactor)
        .ok_or(LoanPoolError::LiquidationThreshold)
}

pub fn read_stored_positions(e: &Env, addr: &Address) -> StoredPositions {
    let key = PoolDataKey::Positions(addr.clone());
    if let Some(positions) = e.storage().persistent().get(&key) {
        extend_persistent(e, &key);
        positions
    } else {
        StoredPositions {
            liabilities: 0,
            collateral_shares: 0,
        }
    }
}

pub fn write_positions(e: &Env, addr: Address, liabilities: i128, collateral: i128) {
    let key = PoolDataKey::Positions(addr.clone());

    let stored = StoredPositions {
        liabilities,
        collateral_shares: collateral,
    };
    e.storage().persistent().set(&key, &stored);
    extend_persistent(e, &key);

    let positions = Positions {
        receivable_shares: 0, // receivable_shares sourced from share token at read time
        liabilities,
        collateral_shares: collateral,
    };
    EventPositionsUpdated { addr, positions }.publish(e)
}

pub fn write_insurance_pool_address(e: &Env, address: Address) {
    let key = PoolDataKey::InsurancePoolAddress;
    e.storage().persistent().set(&key, &address);
    extend_persistent(e, &key);
}

pub fn read_insurance_pool_address(e: &Env) -> Result<Address, LoanPoolError> {
    e.storage()
        .persistent()
        .get(&PoolDataKey::InsurancePoolAddress)
        .ok_or(LoanPoolError::InsurancePoolAddress)
}

pub fn read_total_liabilities(e: &Env) -> i128 {
    e.storage()
        .persistent()
        .get(&PoolDataKey::TotalLiabilitiesTokens)
        .unwrap_or(0)
}

pub fn adjust_total_liabilities(e: &Env, amount: i128) -> Result<i128, LoanPoolError> {
    let key = PoolDataKey::TotalLiabilitiesTokens;
    let current: i128 = e.storage().persistent().get(&key).unwrap_or(0);
    let new_amount = current
        .checked_add(amount)
        .ok_or(LoanPoolError::OverOrUnderFlow)?;
    e.storage().persistent().set(&key, &new_amount);
    extend_persistent(e, &key);
    Ok(new_amount)
}
