use soroban_sdk::{contracttype, symbol_short, Address, Env};

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

pub fn write_loan(e: &Env, user: Address, loan: Loan) {
    let key = LoanManagerDataKey::Loan(user);

    e.storage().persistent().set(&key, &loan);

    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);

    e.events().publish(("Loan", "created"), key);
}

pub fn read_loan(e: &Env, user: Address) -> Option<Loan> {
    e.storage()
        .persistent()
        .get(&LoanManagerDataKey::Loan(user))
}
