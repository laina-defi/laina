use soroban_sdk::{contracttype, symbol_short, vec, Address, Env, Vec};

use crate::error::LoanManagerError;

/* Storage Types */
#[derive(Clone)]
#[contracttype]
pub enum LoanManagerDataKey {
    Admin,
    PoolAddresses,
    Loan(Address),
    LastUpdated,
}

#[derive(Clone)]
#[contracttype]
pub struct Loan {
    pub borrower: Address,
    pub borrowed_amount: i128,
    pub borrowed_from: Address,
    pub collateral_amount: i128,
    pub collateral_from: Address,
    pub health_factor: i128,
    pub unpaid_interest: i128,
    pub last_accrual: i128,
}

/* Ledger Thresholds */
pub(crate) const DAY_IN_LEDGERS: u32 = 17280; // if ledger takes 5 seconds

pub(crate) const POSITIONS_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const POSITIONS_LIFETIME_THRESHOLD: u32 = POSITIONS_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub fn write_admin(e: &Env, admin: &Address) {
    e.storage()
        .persistent()
        .set(&LoanManagerDataKey::Admin, &admin);
    e.events()
        .publish((symbol_short!("admin"), symbol_short!("added")), admin);
}

pub fn admin_exists(e: &Env) -> bool {
    e.storage().persistent().has(&LoanManagerDataKey::Admin)
}

pub fn read_admin(e: &Env) -> Result<Address, LoanManagerError> {
    e.storage()
        .persistent()
        .get(&LoanManagerDataKey::Admin)
        .ok_or(LoanManagerError::AdminNotFound)
}

pub fn append_pool_address(e: &Env, pool_address: Address) {
    let mut pool_addresses = read_pool_addresses(e);
    pool_addresses.push_back(pool_address.clone());
    e.storage()
        .persistent()
        .set(&LoanManagerDataKey::PoolAddresses, &pool_addresses);
    e.events().publish(
        (LoanManagerDataKey::PoolAddresses, symbol_short!("added")),
        &pool_address,
    );
}

pub fn read_pool_addresses(e: &Env) -> Vec<Address> {
    e.storage()
        .persistent()
        .get(&LoanManagerDataKey::PoolAddresses)
        .unwrap_or(vec![&e])
}

pub fn write_loan(e: &Env, user: Address, loan: Loan) {
    let key = LoanManagerDataKey::Loan(user.clone());
    let is_existing = loan_exists(e, user);

    e.storage().persistent().set(&key, &loan);

    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);

    e.events().publish(
        ("Loan", if is_existing { "updated" } else { "created" }),
        key,
    );
}

pub fn loan_exists(e: &Env, user: Address) -> bool {
    e.storage()
        .persistent()
        .has(&LoanManagerDataKey::Loan(user))
}

pub fn read_loan(e: &Env, user: Address) -> Option<Loan> {
    e.storage()
        .persistent()
        .get(&LoanManagerDataKey::Loan(user))
}

pub fn delete_loan(e: &Env, user: Address) {
    e.storage()
        .persistent()
        .remove(&LoanManagerDataKey::Loan(user));
}
