use crate::storage_types::{extend_persistent, PoolDataKey};
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

pub fn read_loan_manager_addr(e: &Env) -> Address {
    let key = PoolDataKey::LoanManagerAddress;

    e.storage().persistent().get(&key).unwrap()
}

pub fn write_currency(e: &Env, currency: Currency) {
    let key = PoolDataKey::Currency;

    e.storage().persistent().set(&key, &currency);
    extend_persistent(e.clone(), &key);
}

pub fn read_currency(e: &Env) -> Currency {
    let key = PoolDataKey::Currency;

    e.storage().persistent().get(&key).unwrap()
}

pub fn write_liquidation_threshold(e: &Env, threshold: i128) {
    let key = PoolDataKey::LiquidationThreshold;

    e.storage().persistent().set(&key, &threshold);
    extend_persistent(e.clone(), &key);
}

pub fn write_total_shares(e: &Env, amount: i128) {
    let key: PoolDataKey = PoolDataKey::TotalShares;

    e.storage().persistent().set(&key, &amount);
    extend_persistent(e.clone(), &key);
}

pub fn read_total_shares(e: &Env) -> i128 {
    let key: PoolDataKey = PoolDataKey::TotalBalance;

    e.storage().persistent().get(&key).unwrap()
}

pub fn change_total_shares(e: &Env, amount: i128) {
    let current_balance = read_total_shares(e);

    write_total_shares(e, amount + current_balance);
}

pub fn write_total_balance(e: &Env, amount: i128) {
    let key: PoolDataKey = PoolDataKey::TotalBalance;

    e.storage().persistent().set(&key, &amount);
    extend_persistent(e.clone(), &key);
}

pub fn read_total_balance(e: &Env) -> i128 {
    let key: PoolDataKey = PoolDataKey::TotalBalance;

    e.storage().persistent().get(&key).unwrap()
}

pub fn change_total_balance(e: &Env, amount: i128) {
    let current_balance = read_total_balance(e);

    write_total_balance(e, amount + current_balance);
}

pub fn write_available_balance(e: &Env, amount: i128) {
    let key: PoolDataKey = PoolDataKey::AvailableBalance;

    e.storage().persistent().set(&key, &amount);
    extend_persistent(e.clone(), &key);
}

pub fn read_available_balance(e: &Env) -> i128 {
    let key: PoolDataKey = PoolDataKey::AvailableBalance;

    e.storage().persistent().get(&key).unwrap()
}

pub fn change_available_balance(e: &Env, amount: i128) {
    let current_balance = read_available_balance(e);

    write_available_balance(e, amount + current_balance);
}

pub fn write_accrual(e: &Env, accrual: i128) {
    let key = PoolDataKey::Accrual;

    e.storage().persistent().set(&key, &accrual);
    extend_persistent(e.clone(), &key);
}

pub fn read_accrual(e: &Env) -> i128 {
    let key = PoolDataKey::Accrual;

    e.storage().persistent().get(&key).unwrap()
}

pub fn write_accrual_last_updated(e: &Env, sequence: u32) {
    let key = PoolDataKey::AccrualLastUpdate;

    e.storage().persistent().set(&key, &sequence);
    extend_persistent(e.clone(), &key);
}

pub fn read_accrual_last_updated(e: &Env) -> u32 {
    let key = PoolDataKey::AccrualLastUpdate;

    e.storage().persistent().get(&key).unwrap()
}
