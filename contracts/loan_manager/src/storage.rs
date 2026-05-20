use soroban_sdk::{contractevent, contracttype, symbol_short, vec, Address, Env, Vec};

use crate::error::LoanManagerError;

/* LAI distribution constants */
pub const LAI_PRECISION: i128 = 1_000_000_000_000; // 1e12, prevents acc_per_share rounding to 0
pub const LAI_TOTAL_LEDGERS: u32 = 31_536_000; // 5 years × 365 days × 86400s / 5s per ledger
pub const LAI_INSURER_BPS: i128 = 7000; // 70% to insurers
pub const LAI_BORROWER_BPS: i128 = 3000; // 30% to borrowers

/* Storage Types */
#[derive(Clone)]
#[contracttype]
pub enum LoanManagerDataKey {
    Admin,
    Oracle,
    PoolAddresses,
    Loan(LoanId),
    LastUpdated,
    LaiConfig,
    LaiNumPools,
    LaiBorrowerPoolState(Address),          // loan_pool → LaiPoolState
    LaiInsurerPoolState(Address),           // insurance_pool → LaiPoolState
    LaiBorrowerUserState(Address, Address), // (pool, user) → LaiUserState
    LaiInsurerUserState(Address, Address),  // (ins_pool, user) → LaiUserState
    LaiInsurancePoolForPool(Address),       // loan_pool → insurance_pool
    InsurancePoolAddresses,
    BadDebtAuction(LoanId),
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AuctionItem {
    pub loan_id: LoanId,
    pub start_ledger: u32,
    pub end_ledger: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct LaiConfig {
    pub token: Address,
    pub start_ledger: u32,
    pub end_ledger: u32,
    pub total_amount: i128,
}

#[derive(Clone)]
#[contracttype]
pub struct LaiPoolState {
    pub acc_per_share: i128, // accumulated LAI per share unit × LAI_PRECISION
    pub last_ledger: u32,
    pub last_known_total: i128, // total shares/liabilities at last update (used in claim to advance acc without cross-contract calls)
}

#[derive(Clone)]
#[contracttype]
pub struct LaiUserState {
    pub reward_debt: i128, // position × acc_per_share at last checkpoint
    pub pending: i128,     // claimable LAI (stroops)
    pub position: i128, // user's last known position (shares for insurers, liabilities for borrowers)
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct LoanId {
    pub borrower_address: Address,
    pub nonce: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct NewLoan {
    pub borrower_address: Address,
    pub borrowed_amount: i128,
    pub borrowed_from: Address,
    pub collateral_shares: i128,
    pub collateral_from: Address,
    pub health_factor: i128,
    pub unpaid_interest: i128,
    pub last_accrual: i128,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Loan {
    pub loan_id: LoanId,
    pub borrowed_amount: i128,
    pub borrowed_from: Address,
    pub collateral_shares: i128,
    pub collateral_from: Address,
    pub health_factor: i128,
    pub unpaid_interest: i128,
    pub last_accrual: i128,
}

/* Contract events */
#[contractevent(topics = ["admin_added"])]
pub struct EventAdminAdded {
    pub admin: Address,
}

#[contractevent(topics = ["oracle_added"])]
pub struct EventOracleAdded {
    pub oracle: Address,
}

#[contractevent(topics = ["pool_address_added"])]
pub struct EventPoolAddressAdded {
    pub pool_address: Address,
}

#[contractevent(topics = ["loan_created"])]
pub struct EventLoanCreated {
    #[topic]
    pub loan_id: LoanId,
    pub loan: Loan,
}

#[contractevent(topics = ["loan_updated"])]
pub struct EventLoanUpdated {
    #[topic]
    pub loan_id: LoanId,
    pub loan: Loan,
}

#[contractevent(topics = ["loan_deleted"])]
pub struct EventLoanDeleted {
    #[topic]
    pub loan_id: LoanId,
}

#[contractevent(topics = ["bad_debt_auction_created"])]
pub struct EventAuctionCreated {
    #[topic]
    pub auction: AuctionItem,
}

#[contractevent(topics = ["bad_debt_auction_deleted"])]
pub struct EventAuctionDeleted {
    #[topic]
    pub auction: AuctionItem,
}

/* Ledger Thresholds */
pub(crate) const DAY_IN_LEDGERS: u32 = 17280; // if ledger takes 5 seconds

pub(crate) const POSITIONS_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const POSITIONS_LIFETIME_THRESHOLD: u32 = POSITIONS_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub fn write_admin(e: &Env, admin: &Address) {
    e.storage()
        .persistent()
        .set(&LoanManagerDataKey::Admin, &admin);
    EventAdminAdded {
        admin: admin.clone(),
    }
    .publish(e);
}

pub fn admin_exists(e: &Env) -> bool {
    e.storage().persistent().has(&LoanManagerDataKey::Admin)
}

pub fn write_bad_debt_auction(e: &Env, auction: AuctionItem) {
    e.storage().persistent().set(
        &LoanManagerDataKey::BadDebtAuction(auction.loan_id.clone()),
        &auction,
    );
    EventAuctionCreated { auction }.publish(e);
}

pub fn read_bad_debt_auction(e: &Env, loan_id: LoanId) -> Result<AuctionItem, LoanManagerError> {
    e.storage()
        .persistent()
        .get(&LoanManagerDataKey::BadDebtAuction(loan_id))
        .ok_or(LoanManagerError::BadDebtAuction)
}

pub fn delete_bad_debt_auction(e: &Env, auction: AuctionItem) {
    e.storage()
        .persistent()
        .remove(&LoanManagerDataKey::BadDebtAuction(auction.loan_id.clone()));
    EventAuctionDeleted { auction }.publish(e);
}

pub fn read_admin(e: &Env) -> Result<Address, LoanManagerError> {
    e.storage()
        .persistent()
        .get(&LoanManagerDataKey::Admin)
        .ok_or(LoanManagerError::AdminNotFound)
}

pub fn write_oracle(e: &Env, oracle: &Address) {
    e.storage()
        .persistent()
        .set(&LoanManagerDataKey::Oracle, &oracle);
    EventOracleAdded {
        oracle: oracle.clone(),
    }
    .publish(e);
}

pub fn read_oracle(e: &Env) -> Result<Address, LoanManagerError> {
    e.storage()
        .persistent()
        .get(&LoanManagerDataKey::Oracle)
        .ok_or(LoanManagerError::OracleNotFound)
}

pub fn append_pool_address(e: &Env, pool_address: Address) {
    let mut pool_addresses = read_pool_addresses(e);
    pool_addresses.push_back(pool_address.clone());
    e.storage()
        .persistent()
        .set(&LoanManagerDataKey::PoolAddresses, &pool_addresses);
    EventPoolAddressAdded {
        pool_address: pool_address.clone(),
    }
    .publish(e);
}

pub fn read_pool_addresses(e: &Env) -> Vec<Address> {
    e.storage()
        .persistent()
        .get(&LoanManagerDataKey::PoolAddresses)
        .unwrap_or(vec![&e])
}

pub fn create_loan(e: &Env, user: Address, new_loan: NewLoan) -> Loan {
    let nonce = get_next_loan_nonce(e, &user);
    let loan_id = LoanId {
        borrower_address: user.clone(),
        nonce,
    };

    let key = LoanManagerDataKey::Loan(loan_id.clone());
    let loan = Loan {
        loan_id: loan_id.clone(),
        borrowed_amount: new_loan.borrowed_amount,
        borrowed_from: new_loan.borrowed_from,
        collateral_shares: new_loan.collateral_shares,
        collateral_from: new_loan.collateral_from,
        health_factor: new_loan.health_factor,
        unpaid_interest: new_loan.unpaid_interest,
        last_accrual: new_loan.last_accrual,
    };
    e.storage().persistent().set(&key, &loan);
    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);

    add_user_loan_id(e, &user, nonce);

    EventLoanCreated {
        loan_id,
        loan: loan.clone(),
    }
    .publish(e);

    loan
}

pub fn write_loan(e: &Env, loan_id: &LoanId, loan: &Loan) {
    let key = LoanManagerDataKey::Loan(loan_id.clone());
    e.storage().persistent().set(&key, loan);
    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
    EventLoanUpdated {
        loan_id: loan_id.clone(),
        loan: loan.clone(),
    }
    .publish(e);
}

pub fn read_loan(e: &Env, loan_id: &LoanId) -> Option<Loan> {
    let key = LoanManagerDataKey::Loan(loan_id.clone());
    e.storage().persistent().get(&key)
}

pub fn read_user_loans(e: &Env, user: &Address) -> Vec<Loan> {
    let nonces = get_user_loan_id_nonces(e, user);
    let mut loans = vec![&e];

    for nonce in nonces.iter() {
        let loan_id = LoanId {
            borrower_address: user.clone(),
            nonce,
        };
        if let Some(loan) = read_loan(e, &loan_id) {
            loans.push_back(loan);
        }
    }

    loans
}

pub fn delete_loan(e: &Env, loan_id: &LoanId) {
    let key = LoanManagerDataKey::Loan(loan_id.clone());
    e.storage().persistent().remove(&key);
    remove_user_loan_id(e, &loan_id.borrower_address, loan_id.nonce);
    EventLoanDeleted {
        loan_id: loan_id.clone(),
    }
    .publish(e);
}

// Increment and return the next loan nonce for a user
fn get_next_loan_nonce(e: &Env, user: &Address) -> u64 {
    let key = (user.clone(), symbol_short!("nonce"));

    let prev_nonce = e.storage().persistent().get(&key).unwrap_or(0);
    let next_nonce = prev_nonce + 1;

    e.storage().persistent().set(&key, &next_nonce);
    next_nonce
}

// Get all loan ID nonces for a user
pub fn get_user_loan_id_nonces(e: &Env, user: &Address) -> Vec<u64> {
    let key = (user.clone(), symbol_short!("ids"));
    e.storage().persistent().get(&key).unwrap_or(vec![&e])
}

// Add a loan ID to user's loan list
pub fn add_user_loan_id(e: &Env, user: &Address, loan_id: u64) {
    let mut loan_ids = get_user_loan_id_nonces(e, user);
    loan_ids.push_back(loan_id);
    let key = (user.clone(), symbol_short!("ids"));
    e.storage().persistent().set(&key, &loan_ids);
}

// Remove a loan ID from user's loan list
pub fn remove_user_loan_id(e: &Env, user: &Address, loan_id: u64) {
    let nonces = get_user_loan_id_nonces(e, user);
    let mut new_nonces = vec![&e];

    for id in nonces.iter() {
        if id != loan_id {
            new_nonces.push_back(id);
        }
    }

    let key = (user.clone(), symbol_short!("ids"));

    if new_nonces.is_empty() {
        e.storage().persistent().remove(&key);
    } else {
        e.storage().persistent().set(&key, &new_nonces);
    }
}

/* LAI distribution storage helpers */

pub fn write_lai_config(e: &Env, config: &LaiConfig) {
    let key = LoanManagerDataKey::LaiConfig;
    e.storage().persistent().set(&key, config);
    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
}

pub fn read_lai_config(e: &Env) -> Option<LaiConfig> {
    let key = LoanManagerDataKey::LaiConfig;
    if let Some(config) = e.storage().persistent().get(&key) {
        e.storage().persistent().extend_ttl(
            &key,
            POSITIONS_LIFETIME_THRESHOLD,
            POSITIONS_BUMP_AMOUNT,
        );
        Some(config)
    } else {
        None
    }
}

pub fn write_lai_num_pools(e: &Env, num_pools: u32) {
    let key = LoanManagerDataKey::LaiNumPools;
    e.storage().persistent().set(&key, &num_pools);
    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
}

pub fn read_lai_num_pools(e: &Env) -> u32 {
    let key = LoanManagerDataKey::LaiNumPools;
    e.storage().persistent().get(&key).unwrap_or(0)
}

pub fn write_lai_borrower_pool_state(e: &Env, pool: &Address, state: &LaiPoolState) {
    let key = LoanManagerDataKey::LaiBorrowerPoolState(pool.clone());
    e.storage().persistent().set(&key, state);
    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
}

pub fn read_lai_borrower_pool_state(e: &Env, pool: &Address) -> Option<LaiPoolState> {
    let key = LoanManagerDataKey::LaiBorrowerPoolState(pool.clone());
    if let Some(state) = e.storage().persistent().get(&key) {
        e.storage().persistent().extend_ttl(
            &key,
            POSITIONS_LIFETIME_THRESHOLD,
            POSITIONS_BUMP_AMOUNT,
        );
        Some(state)
    } else {
        None
    }
}

pub fn write_lai_insurer_pool_state(e: &Env, ins_pool: &Address, state: &LaiPoolState) {
    let key = LoanManagerDataKey::LaiInsurerPoolState(ins_pool.clone());
    e.storage().persistent().set(&key, state);
    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
}

pub fn read_lai_insurer_pool_state(e: &Env, ins_pool: &Address) -> Option<LaiPoolState> {
    let key = LoanManagerDataKey::LaiInsurerPoolState(ins_pool.clone());
    if let Some(state) = e.storage().persistent().get(&key) {
        e.storage().persistent().extend_ttl(
            &key,
            POSITIONS_LIFETIME_THRESHOLD,
            POSITIONS_BUMP_AMOUNT,
        );
        Some(state)
    } else {
        None
    }
}

pub fn write_lai_borrower_user_state(
    e: &Env,
    pool: &Address,
    user: &Address,
    state: &LaiUserState,
) {
    let key = LoanManagerDataKey::LaiBorrowerUserState(pool.clone(), user.clone());
    e.storage().persistent().set(&key, state);
    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
}

pub fn read_lai_borrower_user_state(e: &Env, pool: &Address, user: &Address) -> LaiUserState {
    let key = LoanManagerDataKey::LaiBorrowerUserState(pool.clone(), user.clone());
    if let Some(state) = e.storage().persistent().get(&key) {
        e.storage().persistent().extend_ttl(
            &key,
            POSITIONS_LIFETIME_THRESHOLD,
            POSITIONS_BUMP_AMOUNT,
        );
        state
    } else {
        LaiUserState {
            reward_debt: 0,
            pending: 0,
            position: 0,
        }
    }
}

pub fn write_lai_insurer_user_state(
    e: &Env,
    ins_pool: &Address,
    user: &Address,
    state: &LaiUserState,
) {
    let key = LoanManagerDataKey::LaiInsurerUserState(ins_pool.clone(), user.clone());
    e.storage().persistent().set(&key, state);
    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
}

pub fn read_lai_insurer_user_state(e: &Env, ins_pool: &Address, user: &Address) -> LaiUserState {
    let key = LoanManagerDataKey::LaiInsurerUserState(ins_pool.clone(), user.clone());
    if let Some(state) = e.storage().persistent().get(&key) {
        e.storage().persistent().extend_ttl(
            &key,
            POSITIONS_LIFETIME_THRESHOLD,
            POSITIONS_BUMP_AMOUNT,
        );
        state
    } else {
        LaiUserState {
            reward_debt: 0,
            pending: 0,
            position: 0,
        }
    }
}

pub fn write_lai_insurance_pool_for_pool(e: &Env, pool: &Address, ins_pool: &Address) {
    let key = LoanManagerDataKey::LaiInsurancePoolForPool(pool.clone());
    e.storage().persistent().set(&key, ins_pool);
    e.storage()
        .persistent()
        .extend_ttl(&key, POSITIONS_LIFETIME_THRESHOLD, POSITIONS_BUMP_AMOUNT);
}

pub fn append_insurance_pool_address(e: &Env, ins_pool: Address) {
    let key = LoanManagerDataKey::InsurancePoolAddresses;
    let mut addresses: Vec<Address> = e.storage().persistent().get(&key).unwrap_or(vec![&e]);
    addresses.push_back(ins_pool);
    e.storage().persistent().set(&key, &addresses);
}

pub fn read_insurance_pool_addresses(e: &Env) -> Result<Vec<Address>, LoanManagerError> {
    e.storage()
        .persistent()
        .get(&LoanManagerDataKey::InsurancePoolAddresses)
        .ok_or(LoanManagerError::InsurancePoolAddressesNotFound)
}

pub fn read_lai_insurance_pool_for_pool(e: &Env, pool: &Address) -> Option<Address> {
    let key = LoanManagerDataKey::LaiInsurancePoolForPool(pool.clone());
    if let Some(ins_pool) = e.storage().persistent().get(&key) {
        e.storage().persistent().extend_ttl(
            &key,
            POSITIONS_LIFETIME_THRESHOLD,
            POSITIONS_BUMP_AMOUNT,
        );
        Some(ins_pool)
    } else {
        None
    }
}
