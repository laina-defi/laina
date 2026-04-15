use crate::error::LoanManagerError;
use crate::oracle::{self, Asset};
use crate::storage::{self, Loan, LoanId, LaiConfig, LaiPoolState, LaiUserState, NewLoan};
use crate::storage::{
    LAI_PRECISION, LAI_TOTAL_LEDGERS, LAI_INSURER_BPS, LAI_BORROWER_BPS,
};

use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, Symbol, Vec};

mod loan_pool {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/loan_pool.wasm");
}

#[contract]
struct LoanManager;

#[allow(dead_code)]
#[contractimpl]
impl LoanManager {
    /// Set the admin that's allowed to upgrade the wasm.
    pub fn initialize(
        e: Env,
        admin: Address,
        oracle_address: Address,
    ) -> Result<(), LoanManagerError> {
        if storage::admin_exists(&e) {
            return Err(LoanManagerError::AlreadyInitialized);
        }
        storage::write_admin(&e, &admin);

        storage::write_oracle(&e, &oracle_address);

        Ok(())
    }

    /// Deploy a loan_pool contract, and initialize it.
    pub fn deploy_pool(
        e: Env,
        wasm_hash: BytesN<32>,
        salt: BytesN<32>,
        token_address: Address,
        ticker: Symbol,
        liquidation_threshold: i128,
        pool_token_address: Address,
    ) -> Result<Address, LoanManagerError> {
        // Deploy the contract using the uploaded Wasm with given hash.
        let deployed_address: Address = e
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(wasm_hash, ());

        let admin = storage::read_admin(&e)?;

        admin.require_auth();

        storage::append_pool_address(&e, deployed_address.clone());

        let pool_client = loan_pool::Client::new(&e, &deployed_address);

        let currency = loan_pool::Currency {
            token_address,
            ticker,
        };
        pool_client.initialize(
            &e.current_contract_address(),
            &currency,
            &liquidation_threshold,
            &pool_token_address,
        );

        // If LAI distribution is active, flush existing pool accumulators with old rate
        // before changing num_pools, then initialize state for the new pool.
        if let Some(config) = storage::read_lai_config(&e) {
            Self::flush_all_pool_accumulators(&e, &config);
            let new_num_pools = storage::read_lai_num_pools(&e) + 1;
            storage::write_lai_num_pools(&e, new_num_pools);
            let start = e.ledger().sequence().max(config.start_ledger);
            storage::write_lai_borrower_pool_state(
                &e,
                &deployed_address,
                &LaiPoolState {
                    acc_per_share: 0,
                    last_ledger: start,
                    last_known_total: 0,
                },
            );
        }

        Ok(deployed_address)
    }

    /// Upgrade deployed loan pools and the loan manager WASM.
    pub fn upgrade(
        e: Env,
        new_manager_wasm_hash: BytesN<32>,
        new_pool_wasm_hash: BytesN<32>,
    ) -> Result<(), LoanManagerError> {
        let admin = storage::read_admin(&e)?;
        admin.require_auth();

        storage::read_pool_addresses(&e).iter().for_each(|pool| {
            let pool_client = loan_pool::Client::new(&e, &pool);
            pool_client.upgrade(&new_pool_wasm_hash);
        });

        e.deployer()
            .update_current_contract_wasm(new_manager_wasm_hash);

        Ok(())
    }

    /// Let admin withdraw revenue
    pub fn admin_withdraw_revenue(
        e: &Env,
        amount: i128,
        token_address: Address,
    ) -> Result<(), LoanManagerError> {
        let admin: Address = storage::read_admin(e)?;
        admin.require_auth();

        let token_client = token::Client::new(e, &token_address);
        token_client.transfer(&e.current_contract_address(), &admin, &amount);
        Ok(())
    }

    /// Initialize a new loan
    pub fn create_loan(
        e: Env,
        user: Address,
        borrowed: i128,
        borrowed_from: Address,
        collateral: i128,
        collateral_from: Address,
    ) -> Result<Loan, LoanManagerError> {
        user.require_auth();

        let pool_addresses = storage::read_pool_addresses(&e);
        if !pool_addresses.contains(&borrowed_from) {
            return Err(LoanManagerError::InvalidLoanToken);
        }
        if !pool_addresses.contains(&collateral_from) {
            return Err(LoanManagerError::InvalidCollateralToken);
        }

        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_from);
        let borrow_pool_client = loan_pool::Client::new(&e, &borrowed_from);

        let token_currency = borrow_pool_client.get_currency();
        let collateral_currency = collateral_pool_client.get_currency();
        let health_factor: i128 = Self::calculate_health_factor(
            &e,
            token_currency.ticker,
            borrowed,
            collateral_currency.ticker,
            collateral,
            collateral_from.clone(),
        )?;

        // Health factor is defined as so: 1.0 = 10000000_i128
        const HEALTH_FACTOR_THRESHOLD: i128 = 10000000;
        assert!(
            health_factor > HEALTH_FACTOR_THRESHOLD,
            "Health factor must be over {HEALTH_FACTOR_THRESHOLD} to create a new loan!"
        );

        // Deposit collateral — returns shares issued
        let collateral_shares = collateral_pool_client.deposit_collateral(&user, &collateral);

        // Checkpoint borrower LAI rewards before changing their liability position.
        // For a new loan from this pool, old_liabilities = sum of existing loans from same pool.
        if storage::read_lai_config(&e).is_some() {
            let borrow_pool_state = borrow_pool_client.get_pool_state();
            let total_liabilities = borrow_pool_state.total_balance_tokens - borrow_pool_state.available_balance_tokens;
            let old_liabilities: i128 = storage::read_user_loans(&e, &user)
                .iter()
                .filter(|l| l.borrowed_from == borrowed_from)
                .map(|l| l.borrowed_amount)
                .sum();
            Self::checkpoint_borrower_internal(
                &e,
                &borrowed_from,
                &user,
                old_liabilities,
                old_liabilities + borrowed,
                total_liabilities,
            );
        }

        // Borrow the funds
        let borrowed_amount = borrow_pool_client.borrow(&user, &borrowed);

        let unpaid_interest = 0;

        let new_loan = NewLoan {
            borrower_address: user.clone(),
            borrowed_amount,
            borrowed_from,
            collateral_shares,
            collateral_from,
            health_factor,
            unpaid_interest,
            last_accrual: borrow_pool_client.get_accrual(),
        };

        let loan = storage::create_loan(&e, user.clone(), new_loan);

        Ok(loan)
    }

    /// add interest to a loan
    pub fn add_interest(e: &Env, loan_id: LoanId) -> Result<Loan, LoanManagerError> {
        let Loan {
            borrowed_from,
            collateral_shares,
            borrowed_amount,
            collateral_from,
            unpaid_interest,
            last_accrual,
            ..
        } = Self::get_loan(e, loan_id.clone())?;

        let borrow_pool_client = loan_pool::Client::new(e, &borrowed_from);
        let collateral_pool_client = loan_pool::Client::new(e, &collateral_from);

        let token_ticker = borrow_pool_client.get_currency().ticker;
        let token_collateral_ticker = collateral_pool_client.get_currency().ticker;

        const DECIMAL: i128 = 10000000;

        borrow_pool_client.add_interest_to_accrual();
        let current_accrual = borrow_pool_client.get_accrual();
        let interest_since_update_multiplier = current_accrual
            .checked_mul(DECIMAL)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(last_accrual)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let new_borrowed_amount = borrowed_amount
            .checked_mul(interest_since_update_multiplier)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(DECIMAL)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        // Get current token value of collateral shares for health factor calculation
        let collateral_tokens = collateral_pool_client.shares_to_tokens(&collateral_shares);

        let new_health_factor = Self::calculate_health_factor(
            e,
            token_ticker,
            new_borrowed_amount,
            token_collateral_ticker,
            collateral_tokens,
            collateral_from.clone(),
        )?;

        let borrow_change = new_borrowed_amount
            .checked_sub(borrowed_amount)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;
        let new_unpaid_interest = unpaid_interest
            .checked_add(borrow_change)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        // Update the pool's positions to reflect the increased liabilities from interest
        if borrow_change > 0 {
            borrow_pool_client.increase_liabilities(&loan_id.borrower_address, &borrow_change);
        }

        let updated_loan = Loan {
            loan_id: loan_id.clone(),
            borrowed_from,
            collateral_shares,
            borrowed_amount: new_borrowed_amount,
            collateral_from,
            health_factor: new_health_factor,
            unpaid_interest: new_unpaid_interest,
            last_accrual: current_accrual,
        };

        storage::write_loan(e, &loan_id, &updated_loan);

        Ok(updated_loan)
    }

    pub fn calculate_health_factor(
        e: &Env,
        token_ticker: Symbol,
        token_amount: i128,
        token_collateral_ticker: Symbol,
        token_collateral_amount: i128,
        token_collateral_address: Address,
    ) -> Result<i128, LoanManagerError> {
        const DECIMAL_TO_INT_MULTIPLIER: i128 = 10000000;
        let reflector_address = storage::read_oracle(e)?;
        let reflector_contract = oracle::Client::new(e, &reflector_address);

        // get the price and calculate the value of the collateral
        let collateral_asset = Asset::Other(token_collateral_ticker);

        let collateral_pool_client = loan_pool::Client::new(e, &token_collateral_address);
        let collateral_factor = collateral_pool_client.get_collateral_factor();

        // twap endpoint has been removed so
        // TODO: add own price averaging using the prices endpoint
        let collateral_asset_price = reflector_contract
            .lastprice(&collateral_asset)
            .ok_or(LoanManagerError::NoLastPrice)?;
        let collateral_value = collateral_asset_price
            .price
            .checked_mul(token_collateral_amount)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_mul(collateral_factor)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(DECIMAL_TO_INT_MULTIPLIER)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        // get the price and calculate the value of the borrowed asset
        let borrowed_asset = Asset::Other(token_ticker);
        let asset_price = reflector_contract
            .lastprice(&borrowed_asset)
            .ok_or(LoanManagerError::NoLastPrice)?;
        let borrowed_value = asset_price
            .price
            .checked_mul(token_amount)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let health_factor = collateral_value
            .checked_mul(DECIMAL_TO_INT_MULTIPLIER)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(borrowed_value)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;
        Ok(health_factor)
    }

    pub fn get_oracle(e: Env) -> Result<Address, LoanManagerError> {
        storage::read_oracle(&e)
    }

    /// Get the loans for a specific user
    pub fn get_loans(e: &Env, user: Address) -> Vec<Loan> {
        storage::read_user_loans(e, &user)
    }

    /// Get a single loan by id
    pub fn get_loan(e: &Env, loan_id: LoanId) -> Result<Loan, LoanManagerError> {
        storage::read_loan(e, &loan_id).ok_or(LoanManagerError::LoanNotFound)
    }

    /// Get the price of a token
    pub fn get_price(e: &Env, token: Symbol) -> Result<i128, LoanManagerError> {
        let reflector_address = storage::read_oracle(e)?;
        let reflector_contract = oracle::Client::new(e, &reflector_address);

        let asset = Asset::Other(token);

        let asset_pricedata = reflector_contract
            .lastprice(&asset)
            .ok_or(LoanManagerError::NoLastPrice)?;
        Ok(asset_pricedata.price)
    }

    pub fn repay(e: &Env, loan_id: LoanId, amount: i128) -> Result<(i128, i128), LoanManagerError> {
        let user = loan_id.borrower_address.clone();
        user.require_auth();

        let Loan {
            borrowed_amount,
            borrowed_from,
            collateral_shares,
            collateral_from,
            unpaid_interest,
            last_accrual,
            ..
        } = Self::add_interest(e, loan_id.clone())?;

        assert!(
            amount <= borrowed_amount,
            "Amount can not be greater than borrowed amount!"
        );

        let collateral_pool_client = loan_pool::Client::new(e, &collateral_from);
        let borrow_pool_client = loan_pool::Client::new(e, &borrowed_from);

        // Checkpoint borrower LAI rewards before changing their liability position.
        if storage::read_lai_config(e).is_some() {
            let borrow_pool_state = borrow_pool_client.get_pool_state();
            let total_liabilities = borrow_pool_state.total_balance_tokens - borrow_pool_state.available_balance_tokens;
            Self::checkpoint_borrower_internal(
                e,
                &borrowed_from,
                &user,
                borrowed_amount,
                borrowed_amount - amount,
                total_liabilities,
            );
        }

        borrow_pool_client.repay(&user, &amount, &unpaid_interest);

        let new_unpaid_interest = if amount < unpaid_interest {
            unpaid_interest
                .checked_sub(amount)
                .ok_or(LoanManagerError::OverOrUnderFlow)?
        } else {
            0
        };

        let new_borrowed_amount = borrowed_amount
            .checked_sub(amount)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let collateral_tokens = collateral_pool_client.shares_to_tokens(&collateral_shares);
        let new_health_factor = Self::calculate_health_factor(
            e,
            borrow_pool_client.get_currency().ticker,
            new_borrowed_amount,
            collateral_pool_client.get_currency().ticker,
            collateral_tokens,
            collateral_from.clone(),
        )?;

        storage::write_loan(
            e,
            &loan_id,
            &Loan {
                loan_id: loan_id.clone(),
                borrowed_amount: new_borrowed_amount,
                borrowed_from,
                collateral_shares,
                collateral_from,
                health_factor: new_health_factor,
                unpaid_interest: new_unpaid_interest,
                last_accrual,
            },
        );

        Ok((borrowed_amount, new_borrowed_amount))
    }

    pub fn repay_and_close_manager(
        e: &Env,
        max_allowed_amount: i128,
        loan_id: LoanId,
    ) -> Result<i128, LoanManagerError> {
        let user = loan_id.borrower_address.clone();
        user.require_auth();

        let Loan {
            borrowed_amount,
            borrowed_from,
            collateral_shares,
            collateral_from,
            unpaid_interest,
            ..
        } = Self::add_interest(e, loan_id.clone())?;

        let borrow_pool_client = loan_pool::Client::new(e, &borrowed_from);

        // Checkpoint borrower LAI rewards before closing position (new liabilities = 0).
        if storage::read_lai_config(e).is_some() {
            let borrow_pool_state = borrow_pool_client.get_pool_state();
            let total_liabilities = borrow_pool_state.total_balance_tokens - borrow_pool_state.available_balance_tokens;
            Self::checkpoint_borrower_internal(
                e,
                &borrowed_from,
                &user,
                borrowed_amount,
                0,
                total_liabilities,
            );
        }

        borrow_pool_client.repay_and_close(
            &user,
            &borrowed_amount,
            &max_allowed_amount,
            &unpaid_interest,
        );

        // Withdraw collateral shares — user receives full token value including earned interest
        let collateral_pool_client = loan_pool::Client::new(e, &collateral_from);
        collateral_pool_client.withdraw_collateral(&user, &collateral_shares);

        storage::delete_loan(e, &loan_id);
        Ok(borrowed_amount)
    }

    pub fn liquidate(
        e: Env,
        user: Address,
        loan_id: LoanId,
        amount: i128,
    ) -> Result<Loan, LoanManagerError> {
        user.require_auth();

        let Loan {
            loan_id,
            borrowed_amount,
            borrowed_from,
            collateral_from,
            collateral_shares,
            unpaid_interest,
            last_accrual,
            ..
        } = Self::add_interest(&e, loan_id.clone())?;

        let borrow_pool_client = loan_pool::Client::new(&e, &borrowed_from);
        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_from);

        let borrowed_ticker = borrow_pool_client.get_currency().ticker;
        let collateral_ticker = collateral_pool_client.get_currency().ticker;

        // Get current token value of collateral shares
        let collateral_tokens = collateral_pool_client.shares_to_tokens(&collateral_shares);

        // Check that loan is for sure liquidatable at this moment.
        let health_factor_before_liquidation = Self::calculate_health_factor(
            &e,
            borrowed_ticker.clone(),
            borrowed_amount,
            collateral_ticker.clone(),
            collateral_tokens,
            collateral_from.clone(),
        )?;
        assert!(health_factor_before_liquidation < 10000000);
        // Assert that the liquidation is not more than 50% of loan
        assert!(
            amount
                < (borrowed_amount
                    .checked_div(2)
                    .ok_or(LoanManagerError::OverOrUnderFlow)?)
        );
        // Assert that the liquidation is atleast 1% of loan
        assert!(
            amount
                > (borrowed_amount
                    .checked_div(100)
                    .ok_or(LoanManagerError::OverOrUnderFlow)?)
        );

        let borrowed_price = Self::get_price(&e, borrowed_ticker.clone())?;
        let collateral_price = Self::get_price(&e, collateral_ticker.clone())?;
        let collateral_factor = collateral_pool_client.get_collateral_factor();
        const FIXED_POINT_ONE: i128 = 10_000_000;

        // bonus rate = (1-collateralfactor) / 2 = e.g. 2.5-10 %
        // As multiplier = bonus rate + 1
        let bonus = FIXED_POINT_ONE
            .checked_sub(collateral_factor)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(2_i128)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_add(FIXED_POINT_ONE)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let liquidation_value = amount
            .checked_mul(borrowed_price)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;
        let collateral_amount_bonus = liquidation_value
            .checked_mul(bonus)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(collateral_price)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(10_000_000)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        // Checkpoint borrower LAI rewards before reducing their liability position.
        if storage::read_lai_config(&e).is_some() {
            let borrow_pool_state = borrow_pool_client.get_pool_state();
            let total_liabilities = borrow_pool_state.total_balance_tokens - borrow_pool_state.available_balance_tokens;
            Self::checkpoint_borrower_internal(
                &e,
                &borrowed_from,
                &loan_id.borrower_address,
                borrowed_amount,
                borrowed_amount - amount,
                total_liabilities,
            );
        }

        borrow_pool_client.liquidate(&user, &amount, &unpaid_interest, &loan_id.borrower_address);

        // liquidate_transfer_collateral takes tokens, converts to shares internally, returns shares removed
        let bonus_shares = collateral_pool_client.liquidate_transfer_collateral(
            &user,
            &collateral_amount_bonus,
            &loan_id.borrower_address,
        );

        let new_borrowed_amount = borrowed_amount
            .checked_sub(amount)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;
        let new_collateral_shares = collateral_shares
            .checked_sub(bonus_shares)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let new_collateral_tokens = collateral_pool_client.shares_to_tokens(&new_collateral_shares);
        let new_health_factor = Self::calculate_health_factor(
            &e,
            borrowed_ticker,
            new_borrowed_amount,
            collateral_ticker,
            new_collateral_tokens,
            collateral_from.clone(),
        )?;

        if new_health_factor < health_factor_before_liquidation {
            return Err(LoanManagerError::InvalidLiquidation);
        }

        let new_loan = Loan {
            loan_id: loan_id.clone(),
            borrowed_amount: new_borrowed_amount,
            borrowed_from,
            collateral_from,
            collateral_shares: new_collateral_shares,
            health_factor: new_health_factor,
            unpaid_interest, // Temp
            last_accrual,
        };

        storage::write_loan(&e, &loan_id, &new_loan);

        Ok(new_loan)
    }

    /// Add more collateral to an existing loan, improving its health factor.
    /// `amount` is the token amount to deposit as additional collateral.
    pub fn add_collateral(e: Env, loan_id: LoanId, amount: i128) -> Result<Loan, LoanManagerError> {
        loan_id.borrower_address.require_auth();

        let mut loan = Self::add_interest(&e, loan_id.clone())?;

        let collateral_pool_client = loan_pool::Client::new(&e, &loan.collateral_from);
        let borrow_pool_client = loan_pool::Client::new(&e, &loan.borrowed_from);

        let new_shares =
            collateral_pool_client.deposit_collateral(&loan_id.borrower_address, &amount);
        loan.collateral_shares = loan
            .collateral_shares
            .checked_add(new_shares)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let collateral_tokens = collateral_pool_client.shares_to_tokens(&loan.collateral_shares);
        loan.health_factor = Self::calculate_health_factor(
            &e,
            borrow_pool_client.get_currency().ticker,
            loan.borrowed_amount,
            collateral_pool_client.get_currency().ticker,
            collateral_tokens,
            loan.collateral_from.clone(),
        )?;

        storage::write_loan(&e, &loan_id, &loan);
        Ok(loan)
    }

    /// Remove collateral from an existing loan, decreasing its health factor.
    /// `amount` is the token amount to withdraw. Fails if health factor would drop below 1.0.
    pub fn remove_collateral(
        e: Env,
        loan_id: LoanId,
        amount: i128,
    ) -> Result<Loan, LoanManagerError> {
        loan_id.borrower_address.require_auth();

        let mut loan = Self::add_interest(&e, loan_id.clone())?;

        let collateral_pool_client = loan_pool::Client::new(&e, &loan.collateral_from);
        let borrow_pool_client = loan_pool::Client::new(&e, &loan.borrowed_from);

        let shares_to_remove = collateral_pool_client.tokens_to_shares(&amount);
        let new_collateral_shares = loan
            .collateral_shares
            .checked_sub(shares_to_remove)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let new_collateral_tokens = collateral_pool_client.shares_to_tokens(&new_collateral_shares);
        let new_health_factor = Self::calculate_health_factor(
            &e,
            borrow_pool_client.get_currency().ticker,
            loan.borrowed_amount,
            collateral_pool_client.get_currency().ticker,
            new_collateral_tokens,
            loan.collateral_from.clone(),
        )?;

        const HEALTH_FACTOR_THRESHOLD: i128 = 10_000_000;
        assert!(
            new_health_factor > HEALTH_FACTOR_THRESHOLD,
            "Removing collateral would make the loan liquidatable!"
        );

        collateral_pool_client.withdraw_collateral(&loan_id.borrower_address, &shares_to_remove);

        loan.collateral_shares = new_collateral_shares;
        loan.health_factor = new_health_factor;

        storage::write_loan(&e, &loan_id, &loan);
        Ok(loan)
    }

    /// Set the insurance pool address for a given loan pool.
    /// Admin only. Delegates to the loan pool's `set_insurance_pool`.
    pub fn set_insurance_pool(
        e: Env,
        pool_addr: Address,
        insurance_pool_addr: Address,
    ) -> Result<(), LoanManagerError> {
        let admin = storage::read_admin(&e)?;
        admin.require_auth();

        let pool_client = loan_pool::Client::new(&e, &pool_addr);
        pool_client.set_insurance_pool(&insurance_pool_addr);

        // Store the loan_pool → insurance_pool mapping for LAI distribution
        storage::write_lai_insurance_pool_for_pool(&e, &pool_addr, &insurance_pool_addr);

        // Initialize insurer pool state if LAI distribution is active
        if let Some(config) = storage::read_lai_config(&e) {
            if storage::read_lai_insurer_pool_state(&e, &insurance_pool_addr).is_none() {
                let start = e.ledger().sequence().max(config.start_ledger);
                storage::write_lai_insurer_pool_state(
                    &e,
                    &insurance_pool_addr,
                    &LaiPoolState {
                        acc_per_share: 0,
                        last_ledger: start,
                        last_known_total: 0,
                    },
                );
            }
        }

        Ok(())
    }

    /// Write off bad debt for a loan: calls the loan pool to burn insurance coverage,
    /// adjusts pool accounting, and deletes the loan record.
    /// Admin only.
    pub fn handle_bad_debt(e: Env, loan_id: LoanId) -> Result<(), LoanManagerError> {
        let admin = storage::read_admin(&e)?;
        admin.require_auth();

        let loan = storage::read_loan(&e, &loan_id).ok_or(LoanManagerError::LoanNotFound)?;

        let borrow_pool_client = loan_pool::Client::new(&e, &loan.borrowed_from);
        borrow_pool_client.write_off_bad_debt(&loan.borrowed_amount);

        storage::delete_loan(&e, &loan_id);

        Ok(())
    }

    /// Initialize the LAI liquidity-mining distribution.
    /// Transfers 50M LAI must already be in this contract's balance before calling.
    /// Admin only.
    pub fn initialize_lai_distribution(
        e: Env,
        token: Address,
        start_ledger: u32,
    ) -> Result<(), LoanManagerError> {
        let admin = storage::read_admin(&e)?;
        admin.require_auth();

        const TOTAL_AMOUNT: i128 = 50_000_000 * 10_000_000; // 50M with 7 decimals
        let end_ledger = start_ledger + LAI_TOTAL_LEDGERS;

        let config = LaiConfig {
            token,
            start_ledger,
            end_ledger,
            total_amount: TOTAL_AMOUNT,
        };
        storage::write_lai_config(&e, &config);

        // Initialize pool states for all already-registered pools
        let pool_addresses = storage::read_pool_addresses(&e);
        let num_pools = pool_addresses.len() as u32;
        storage::write_lai_num_pools(&e, num_pools);

        for pool in pool_addresses.iter() {
            storage::write_lai_borrower_pool_state(
                &e,
                &pool,
                &LaiPoolState {
                    acc_per_share: 0,
                    last_ledger: start_ledger,
                    last_known_total: 0,
                },
            );
            if let Some(ins_pool) = storage::read_lai_insurance_pool_for_pool(&e, &pool) {
                storage::write_lai_insurer_pool_state(
                    &e,
                    &ins_pool,
                    &LaiPoolState {
                        acc_per_share: 0,
                        last_ledger: start_ledger,
                        last_known_total: 0,
                    },
                );
            }
        }

        Ok(())
    }

    /// Called by insurance_pool contracts when a user's share position changes.
    /// Caller must be a registered insurance pool.
    pub fn checkpoint_insurer_reward(
        e: Env,
        insurance_pool: Address,
        user: Address,
        old_shares: i128,
        new_shares: i128,
        total_shares: i128,
    ) -> Result<(), LoanManagerError> {
        insurance_pool.require_auth();

        let config = match storage::read_lai_config(&e) {
            Some(c) => c,
            None => return Ok(()), // LAI not initialized — ignore silently
        };

        Self::checkpoint_insurer_internal(
            &e,
            &config,
            &insurance_pool,
            &user,
            old_shares,
            new_shares,
            total_shares,
        );

        Ok(())
    }

    /// Claim all accumulated LAI rewards for a user (both borrower and insurer sides).
    /// Returns the total amount of LAI transferred.
    pub fn claim_lai_rewards(e: Env, user: Address) -> Result<i128, LoanManagerError> {
        user.require_auth();

        let config = match storage::read_lai_config(&e) {
            Some(c) => c,
            None => return Ok(0),
        };

        let mut total_pending: i128 = 0;
        let pool_addresses = storage::read_pool_addresses(&e);

        for pool in pool_addresses.iter() {
            // --- Borrower side ---
            if let Some(mut pool_state) = storage::read_lai_borrower_pool_state(&e, &pool) {
                // Compute current liabilities from loan records
                let user_liabilities: i128 = storage::read_user_loans(&e, &user)
                    .iter()
                    .filter(|l| l.borrowed_from == pool)
                    .map(|l| l.borrowed_amount)
                    .sum();

                // Advance accumulator using last known total
                Self::advance_accumulator(
                    &e,
                    &config,
                    &mut pool_state,
                    LAI_BORROWER_BPS,
                );

                let user_state = storage::read_lai_borrower_user_state(&e, &pool, &user);
                let earned = user_liabilities
                    .checked_mul(pool_state.acc_per_share)
                    .unwrap_or(0)
                    / LAI_PRECISION
                    - user_state.reward_debt;
                let earned = earned.max(0);

                storage::write_lai_borrower_pool_state(&e, &pool, &pool_state);
                storage::write_lai_borrower_user_state(
                    &e,
                    &pool,
                    &user,
                    &LaiUserState {
                        reward_debt: user_liabilities
                            .checked_mul(pool_state.acc_per_share)
                            .unwrap_or(0)
                            / LAI_PRECISION,
                        pending: 0,
                        position: user_liabilities,
                    },
                );
                total_pending += user_state.pending + earned;
            }

            // --- Insurer side ---
            if let Some(ins_pool) = storage::read_lai_insurance_pool_for_pool(&e, &pool) {
                if let Some(mut ins_state) = storage::read_lai_insurer_pool_state(&e, &ins_pool) {
                    let user_ins_state =
                        storage::read_lai_insurer_user_state(&e, &ins_pool, &user);
                    let user_shares = user_ins_state.position;

                    Self::advance_accumulator(
                        &e,
                        &config,
                        &mut ins_state,
                        LAI_INSURER_BPS,
                    );

                    let earned = user_shares
                        .checked_mul(ins_state.acc_per_share)
                        .unwrap_or(0)
                        / LAI_PRECISION
                        - user_ins_state.reward_debt;
                    let earned = earned.max(0);

                    storage::write_lai_insurer_pool_state(&e, &ins_pool, &ins_state);
                    storage::write_lai_insurer_user_state(
                        &e,
                        &ins_pool,
                        &user,
                        &LaiUserState {
                            reward_debt: user_shares
                                .checked_mul(ins_state.acc_per_share)
                                .unwrap_or(0)
                                / LAI_PRECISION,
                            pending: 0,
                            position: user_shares,
                        },
                    );
                    total_pending += user_ins_state.pending + earned;
                }
            }
        }

        if total_pending > 0 {
            let lai_client = token::Client::new(&e, &config.token);
            lai_client.transfer(&e.current_contract_address(), &user, &total_pending);
        }

        Ok(total_pending)
    }

    // ── Internal LAI helpers ──────────────────────────────────────────────────

    /// Advance `pool_state.acc_per_share` from `last_ledger` to current ledger (capped at end_ledger).
    /// Uses `last_known_total` stored in pool_state. Mutates pool_state in place; caller must write it back.
    fn advance_accumulator(
        e: &Env,
        config: &LaiConfig,
        pool_state: &mut LaiPoolState,
        side_bps: i128,
    ) {
        let num_pools = storage::read_lai_num_pools(e) as i128;
        if num_pools == 0 {
            return;
        }

        let current_ledger = e.ledger().sequence().min(config.end_ledger);
        if current_ledger <= pool_state.last_ledger {
            pool_state.last_ledger = current_ledger;
            return;
        }

        let elapsed = (current_ledger - pool_state.last_ledger) as i128;
        // emission for this side (insurer or borrower) per pool per ledger
        let emission_per_ledger = config.total_amount / num_pools / LAI_TOTAL_LEDGERS as i128
            * side_bps / 10_000;

        if pool_state.last_known_total > 0 {
            let new_rewards = emission_per_ledger * elapsed;
            pool_state.acc_per_share += new_rewards * LAI_PRECISION / pool_state.last_known_total;
        }
        // If last_known_total == 0: emissions are lost (no users in pool), accumulator stays unchanged.

        pool_state.last_ledger = current_ledger;
    }

    /// Checkpoint a user's borrower reward for a given pool.
    fn checkpoint_borrower_internal(
        e: &Env,
        pool: &Address,
        user: &Address,
        old_liabilities: i128,
        new_liabilities: i128,
        total_liabilities: i128,
    ) {
        let config = match storage::read_lai_config(e) {
            Some(c) => c,
            None => return,
        };
        let mut pool_state = match storage::read_lai_borrower_pool_state(e, pool) {
            Some(s) => s,
            None => return,
        };

        // Advance accumulator using the OLD total (correct: rewards during elapsed ledgers
        // were earned by whoever held positions then, not the post-event total).
        pool_state.last_known_total = total_liabilities;
        Self::advance_accumulator(e, &config, &mut pool_state, LAI_BORROWER_BPS);
        // Update to the POST-event total so the next checkpoint advances correctly.
        // Without this, last_known_total stays at the old value (0 for first borrower),
        // causing advance_accumulator to skip every future call.
        pool_state.last_known_total = total_liabilities + (new_liabilities - old_liabilities);

        let user_state = storage::read_lai_borrower_user_state(e, pool, user);
        let earned = old_liabilities
            .checked_mul(pool_state.acc_per_share)
            .unwrap_or(0)
            / LAI_PRECISION
            - user_state.reward_debt;
        let earned = earned.max(0);

        storage::write_lai_borrower_pool_state(e, pool, &pool_state);
        storage::write_lai_borrower_user_state(
            e,
            pool,
            user,
            &LaiUserState {
                reward_debt: new_liabilities
                    .checked_mul(pool_state.acc_per_share)
                    .unwrap_or(0)
                    / LAI_PRECISION,
                pending: user_state.pending + earned,
                position: new_liabilities,
            },
        );
    }

    /// Checkpoint a user's insurer reward for a given insurance pool.
    fn checkpoint_insurer_internal(
        e: &Env,
        config: &LaiConfig,
        ins_pool: &Address,
        user: &Address,
        old_shares: i128,
        new_shares: i128,
        total_shares: i128,
    ) {
        let mut pool_state = match storage::read_lai_insurer_pool_state(e, ins_pool) {
            Some(s) => s,
            None => return,
        };

        // Advance using the OLD total, then update to the POST-event total.
        // Same fix as checkpoint_borrower_internal: without updating after advance,
        // last_known_total stays 0 for the first depositor and rewards are lost forever.
        pool_state.last_known_total = total_shares;
        Self::advance_accumulator(e, config, &mut pool_state, LAI_INSURER_BPS);
        pool_state.last_known_total = total_shares + (new_shares - old_shares);

        let user_state = storage::read_lai_insurer_user_state(e, ins_pool, user);
        let earned = old_shares
            .checked_mul(pool_state.acc_per_share)
            .unwrap_or(0)
            / LAI_PRECISION
            - user_state.reward_debt;
        let earned = earned.max(0);

        storage::write_lai_insurer_pool_state(e, ins_pool, &pool_state);
        storage::write_lai_insurer_user_state(
            e,
            ins_pool,
            user,
            &LaiUserState {
                reward_debt: new_shares
                    .checked_mul(pool_state.acc_per_share)
                    .unwrap_or(0)
                    / LAI_PRECISION,
                pending: user_state.pending + earned,
                position: new_shares,
            },
        );
    }

    /// Flush all existing pool accumulators with the current emission rate.
    /// Must be called before LaiNumPools changes (e.g. on deploy_pool).
    fn flush_all_pool_accumulators(e: &Env, config: &LaiConfig) {
        for pool in storage::read_pool_addresses(e).iter() {
            if let Some(mut pool_state) = storage::read_lai_borrower_pool_state(e, &pool) {
                Self::advance_accumulator(e, config, &mut pool_state, LAI_BORROWER_BPS);
                storage::write_lai_borrower_pool_state(e, &pool, &pool_state);
            }
            if let Some(ins_pool) = storage::read_lai_insurance_pool_for_pool(e, &pool) {
                if let Some(mut ins_state) = storage::read_lai_insurer_pool_state(e, &ins_pool) {
                    Self::advance_accumulator(e, config, &mut ins_state, LAI_INSURER_BPS);
                    storage::write_lai_insurer_pool_state(e, &ins_pool, &ins_state);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::contract::loan_pool::{PoolState, PoolStatus, Positions};

    use super::*;
    use loan_pool::Currency;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::{Client as TokenClient, StellarAssetClient},
        xdr::ToXdr,
        Env,
    };
    mod loan_manager {
        soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/loan_manager.wasm");
    }

    mod share_token_wasm {
        soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/token.wasm");
    }

    #[test]
    fn initialize() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);
        let manager_addr = e.register(LoanManager, ());
        let manager_client = LoanManagerClient::new(&e, &manager_addr);

        assert!(manager_client.try_initialize(&admin, &oracle).is_ok());
    }

    #[test]
    fn cannot_re_initialize() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);

        let contract_id = e.register(LoanManager, ());
        let client = LoanManagerClient::new(&e, &contract_id);

        client.initialize(&admin, &oracle);

        assert!(client.try_initialize(&admin, &oracle).is_err())
    }

    #[test]
    fn deploy_pool() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths();

        // ACT
        // Deploy contract using loan_manager as factory
        let TestEnv {
            pool_usdc_client,
            pool_eurc_client,
            ..
        } = setup_test_env(&e);

        // ASSERT
        // No authorizations needed - the contract acts as a factory.
        // assert_eq!(e.auths(), &[]);

        // Invoke contract to check that it is initialized.
        let usdc_balance = pool_usdc_client.get_contract_balance();
        assert_eq!(usdc_balance, 1000);
        let eurc_balance = pool_eurc_client.get_contract_balance();
        assert_eq!(eurc_balance, 1000);
    }

    #[test]
    fn upgrade_manager_and_pool() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths();

        let TestEnv { manager_client, .. } = setup_test_env(&e);
        let manager_wasm_hash = e.deployer().upload_contract_wasm(loan_pool::WASM);
        let pool_wasm_hash = e.deployer().upload_contract_wasm(loan_pool::WASM);

        // ACT
        manager_client.upgrade(&manager_wasm_hash, &pool_wasm_hash);
    }

    #[test]
    fn cannot_create_loan_untrusted_loan_pool() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths();
        let TestEnv {
            admin,
            user,
            manager_addr,
            manager_client,
            pool_xlm_addr,
            ..
        } = setup_test_env(&e);

        // Set up a pool that's not trusted by loan manager
        let ticker = Symbol::new(&e, "XLM");
        let token_address = e
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let pool_addr = e.register(loan_pool::WASM, ());
        let pool_client = loan_pool::Client::new(&e, &pool_addr);
        let dummy_share_token = Address::generate(&e);
        pool_client.initialize(
            &manager_addr,
            &Currency {
                ticker,
                token_address,
            },
            &8_000_000,
            &dummy_share_token,
        );

        // ACT
        let res = manager_client.try_create_loan(&user, &10, &pool_xlm_addr, &100, &pool_addr);
        assert!(res.is_err());
    }

    #[test]
    fn cannot_create_loan_untrusted_collateral_pool() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths();
        let TestEnv {
            admin,
            user,
            manager_addr,
            manager_client,
            pool_xlm_addr,
            ..
        } = setup_test_env(&e);

        // Set up a pool that's not trusted by loan manager
        let ticker = Symbol::new(&e, "XLM");
        let token_address = e
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let pool_addr = e.register(loan_pool::WASM, ());
        let pool_client = loan_pool::Client::new(&e, &pool_addr);
        let dummy_share_token = Address::generate(&e);
        pool_client.initialize(
            &manager_addr,
            &Currency {
                ticker,
                token_address,
            },
            &8_000_000,
            &dummy_share_token,
        );

        // ACT
        let res = manager_client.try_create_loan(&user, &10, &pool_addr, &100, &pool_xlm_addr);
        assert!(res.is_err());
    }

    #[test]
    fn withdraw_revenue_as_admin() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 1_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 1_000_001;
        });

        let TestEnv {
            user,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            xlm_token_client,
            usdc_token_client,
            manager_addr,
            pool_xlm_client,
            xlm_asset_client,
            usdc_asset_client,
            admin,
            reflector_addr,
            ..
        } = setup_test_env(&e);

        xlm_asset_client.mint(&admin, &9_001);
        pool_xlm_client.deposit(&admin, &9_001);
        pool_xlm_client.update_status(&9_001);
        usdc_asset_client.mint(&user, &100_000);

        // Create a loan.
        let loan =
            manager_client.create_loan(&user, &1_000, &pool_xlm_addr, &100_000, &pool_usdc_addr);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        e.register_at(&reflector_addr, oracle::WASM, ());

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 2_000);
        assert_eq!(usdc_token_client.balance(&user), 0);

        let user_loan = manager_client.get_loan(&loan.loan_id);

        assert_eq!(user_loan.borrowed_amount, 1_000);
        assert_eq!(user_loan.collateral_shares, 100_000);

        manager_client.repay(&loan.loan_id, &100);
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000 + 1;
        });

        let user_loan = manager_client.get_loan(&loan.loan_id);

        assert_eq!(user_loan.borrowed_amount, 929);
        assert_eq!(user_loan.collateral_shares, 100_000);
        assert_eq!(xlm_token_client.balance(&manager_addr), 2);

        manager_client
            .admin_withdraw_revenue(&1_i128, &pool_xlm_client.get_currency().token_address);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #12)")]
    fn withdraw_new_user_after_pool_has_yield() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 10_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 1_000_001;
        });

        let TestEnv {
            user,
            admin,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            xlm_token_client,
            usdc_token_client,
            pool_usdc_client,
            reflector_addr,
            usdc_asset_client,
            ..
        } = setup_test_env(&e);

        // ACT
        // Create a loan.
        let mut loan =
            manager_client.create_loan(&user, &100, &pool_usdc_addr, &500, &pool_xlm_addr);
        assert_eq!(pool_usdc_client.get_available_balance(), 900);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        e.register_at(&reflector_addr, oracle::WASM, ());

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 500);
        assert_eq!(usdc_token_client.balance(&user), 100);

        loan = manager_client.get_loan(&loan.loan_id);

        assert_eq!(loan.borrowed_amount, 100);
        assert_eq!(loan.collateral_shares, 500);

        manager_client.repay(&loan.loan_id, &50);
        loan = manager_client.get_loan(&loan.loan_id);
        assert_eq!(loan.borrowed_amount, 52);

        assert_eq!((52, 2), manager_client.repay(&loan.loan_id, &50));
        assert_eq!(1000, pool_usdc_client.get_available_balance());
        assert_eq!(1002, pool_usdc_client.get_contract_balance());
        assert_eq!(1000, pool_usdc_client.get_total_balance_shares());

        // Create a new user that should not be able to withdraw more than what they have deposited even if the pool already has interest
        let new_user = Address::generate(&e);
        usdc_asset_client.mint(&new_user, &1_000);

        pool_usdc_client.deposit(&new_user, &1000);
        assert_eq!(2002, pool_usdc_client.get_contract_balance());
        let positions_new_user = Positions {
            collateral_shares: 0,
            liabilities: 0,
            receivable_shares: 998,
        };
        assert_eq!(
            positions_new_user,
            pool_usdc_client.get_user_positions(&new_user)
        );

        let test_positions_admin = Positions {
            collateral_shares: 0,
            liabilities: 0,
            receivable_shares: 1000,
        };
        assert_eq!(
            test_positions_admin,
            pool_usdc_client.get_user_positions(&admin)
        );
        let pool_state = PoolState {
            annual_interest_rate: 200887,
            available_balance_tokens: 2000,
            total_balance_shares: 1998,
            total_balance_tokens: 2002,
            pool_status: PoolStatus::Healthy,
        };
        assert_eq!(pool_state, pool_usdc_client.get_pool_state());

        // Should panic because the user has no xlm to withdraw
        pool_usdc_client.withdraw(&new_user, &1002);
    }

    #[test]
    fn create_loan() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let TestEnv {
            user,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            pool_eurc_addr,
            xlm_token_client,
            usdc_token_client,
            eurc_token_client,
            ..
        } = setup_test_env(&e);

        // ACT
        manager_client.create_loan(&user, &10, &pool_usdc_addr, &100, &pool_xlm_addr);
        manager_client.create_loan(&user, &30, &pool_eurc_addr, &300, &pool_xlm_addr);

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 600);
        assert_eq!(usdc_token_client.balance(&user), 10);
        assert_eq!(eurc_token_client.balance(&user), 30);

        let loans = manager_client.get_loans(&user);
        assert_eq!(loans.len(), 2);

        let loan_usdc = loans.get(0).unwrap();
        assert_eq!(loan_usdc.borrowed_amount, 10);
        assert_eq!(loan_usdc.collateral_shares, 100);
        assert_eq!(loan_usdc.borrowed_from, pool_usdc_addr);
        assert_eq!(loan_usdc.collateral_from, pool_xlm_addr);

        let loan_eurc = loans.get(1).unwrap();
        assert_eq!(loan_eurc.borrowed_amount, 30);
        assert_eq!(loan_eurc.collateral_shares, 300);
        assert_eq!(loan_eurc.borrowed_from, pool_eurc_addr);
        assert_eq!(loan_eurc.collateral_from, pool_xlm_addr);
    }

    #[test]
    fn add_interest() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 10_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 1_000_001;
        });

        let TestEnv {
            user,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            xlm_token_client,
            usdc_token_client,
            reflector_addr,
            ..
        } = setup_test_env(&e);

        // ACT

        // Create a loan.
        let mut loan =
            manager_client.create_loan(&user, &100, &pool_usdc_addr, &1000, &pool_xlm_addr);

        // Here borrowed amount should be the same as time has not moved. add_interest() is only called to store the LastUpdate sequence number.
        assert_eq!(loan.borrowed_amount, 100);
        assert_eq!(loan.health_factor, 80_000_000);
        assert_eq!(xlm_token_client.balance(&user), 0);
        assert_eq!(usdc_token_client.balance(&user), 100);

        // Move time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        e.register_at(&reflector_addr, oracle::WASM, ());

        loan = manager_client.add_interest(&loan.loan_id);

        assert_eq!(loan.borrowed_amount, 102);
        assert_eq!(loan.health_factor, 78_431_372);
        assert_eq!(loan.collateral_shares, 1000);
    }

    #[test]
    fn repay() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 1_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 1_000_001;
        });

        let TestEnv {
            user,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            pool_eurc_addr,
            pool_usdc_client,
            pool_eurc_client,
            xlm_token_client,
            usdc_token_client,
            reflector_addr,
            eurc_token_client,
            ..
        } = setup_test_env(&e);

        // ACT
        // Create a loan.
        let mut loan_usdc =
            manager_client.create_loan(&user, &100, &pool_usdc_addr, &500, &pool_xlm_addr);
        let mut loan_eurc =
            manager_client.create_loan(&user, &100, &pool_eurc_addr, &500, &pool_xlm_addr);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        e.register_at(&reflector_addr, oracle::WASM, ());

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 0);
        assert_eq!(usdc_token_client.balance(&user), 100);
        assert_eq!(eurc_token_client.balance(&user), 100);

        loan_usdc = manager_client.get_loan(&loan_usdc.loan_id);
        assert_eq!(loan_usdc.borrowed_amount, 100);
        assert_eq!(loan_usdc.collateral_shares, 500);

        manager_client.repay(&loan_usdc.loan_id, &50);
        loan_usdc = manager_client.get_loan(&loan_usdc.loan_id);
        assert_eq!(loan_usdc.borrowed_amount, 52);
        assert_eq!(loan_usdc.collateral_shares, 500);

        assert_eq!((52, 2), manager_client.repay(&loan_usdc.loan_id, &50));
        assert_eq!(1000, pool_usdc_client.get_available_balance());
        assert_eq!(1002, pool_usdc_client.get_contract_balance());
        assert_eq!(1000, pool_usdc_client.get_total_balance_shares());

        loan_eurc = manager_client.get_loan(&loan_eurc.loan_id);
        assert_eq!(loan_eurc.borrowed_amount, 100);
        assert_eq!(loan_eurc.collateral_shares, 500);
        assert_eq!(900, pool_eurc_client.get_available_balance());
        assert_eq!(1000, pool_eurc_client.get_contract_balance());
        assert_eq!(1000, pool_eurc_client.get_total_balance_shares());
    }

    #[test]
    fn repay_and_close() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 1_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 1_000_001;
        });

        let TestEnv {
            user,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            xlm_token_client,
            usdc_token_client,
            reflector_addr,
            pool_usdc_client,
            usdc_asset_client,
            ..
        } = setup_test_env(&e);

        // ACT
        // Create a loan.
        let loan = manager_client.create_loan(&user, &100, &pool_usdc_addr, &300, &pool_xlm_addr);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        e.register_at(&reflector_addr, oracle::WASM, ());

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 700);
        assert_eq!(usdc_token_client.balance(&user), 100);

        let loans = manager_client.get_loans(&user);
        assert_eq!(loans.len(), 1);

        // mint the user some money so they can repay.
        usdc_asset_client.mint(&user, &45);
        manager_client.repay_and_close_manager(&145, &loan.loan_id);

        let loans = manager_client.get_loans(&user);
        assert_eq!(loans.len(), 0);
        assert_eq!(1002, pool_usdc_client.get_available_balance());
        assert_eq!(1002, pool_usdc_client.get_contract_balance());
        assert_eq!(1000, pool_usdc_client.get_total_balance_shares());
    }

    #[test]
    fn repay_and_close_with_multiple_loans() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 1_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 1_000_001;
        });

        let TestEnv {
            user,
            manager_client,
            usdc_asset_client,
            pool_usdc_client,
            pool_xlm_addr,
            pool_usdc_addr,
            pool_eurc_addr,
            usdc_token_client,
            reflector_addr,
            ..
        } = setup_test_env(&e);

        // ACT
        let mut usdc_loan =
            manager_client.create_loan(&user, &100, &pool_usdc_addr, &300, &pool_xlm_addr);
        manager_client.create_loan(&user, &100, &pool_eurc_addr, &300, &pool_xlm_addr);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // ASSERT
        // A new instance of reflector mock needs to be created, they only live for one ledger.
        e.register_at(&reflector_addr, oracle::WASM, ());

        let loans = manager_client.get_loans(&user);
        assert_eq!(loans.len(), 2);
        usdc_loan = manager_client.get_loan(&usdc_loan.loan_id);

        assert_eq!(usdc_loan.borrowed_amount, 100);
        assert_eq!(usdc_loan.collateral_shares, 300);

        // mint the user some money so they can repay.
        usdc_asset_client.mint(&user, &45);
        assert_eq!(
            102,
            manager_client
                .repay_and_close_manager(&(usdc_loan.borrowed_amount + 45), &usdc_loan.loan_id)
        );

        assert_eq!(1002, pool_usdc_client.get_available_balance());
        assert_eq!(1002, pool_usdc_client.get_contract_balance());
        assert_eq!(1000, pool_usdc_client.get_total_balance_shares());
        assert_eq!(43, usdc_token_client.balance(&user));

        // The eurc loan should still be there after repaying the usdc loan
        let loans = manager_client.get_loans(&user);
        assert_eq!(loans.len(), 1);
        let eurc_loan = loans.get(0).unwrap();
        assert_eq!(eurc_loan.borrowed_amount, 100);
        assert_eq!(eurc_loan.collateral_shares, 300);
    }

    #[test]
    #[should_panic(expected = "Amount can not be greater than borrowed amount!")]
    fn repay_more_than_borrowed() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        let TestEnv {
            user,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            ..
        } = setup_test_env(&e);

        // ACT
        // Create a loan.
        let loan = manager_client.create_loan(&user, &100, &pool_usdc_addr, &1000, &pool_xlm_addr);

        manager_client.repay(&loan.loan_id, &2_000);
    }

    #[test]
    fn liquidate() {
        // ARRANGE
        let e = Env::default();

        e.mock_all_auths_allowing_non_root_auth();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 1_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 1_000_001;
        });

        let TestEnv {
            admin,
            user,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            pool_eurc_addr,
            pool_usdc_client,
            pool_eurc_client,
            eurc_asset_client,
            xlm_asset_client,
            usdc_asset_client,
            reflector_addr,
            ..
        } = setup_test_env(&e);

        // print more money
        usdc_asset_client.mint(&admin, &9_001);
        eurc_asset_client.mint(&admin, &9_001);
        xlm_asset_client.mint(&user, &30_000);
        pool_usdc_client.deposit(&admin, &9_001);
        pool_eurc_client.deposit(&admin, &9_001);

        // ACT
        // Create two loans, one to liquidate.
        let mut usdc_loan =
            manager_client.create_loan(&user, &10_000, &pool_usdc_addr, &12_505, &pool_xlm_addr);
        let mut eurc_loan =
            manager_client.create_loan(&user, &10_000, &pool_eurc_addr, &12_505, &pool_xlm_addr);

        manager_client.add_interest(&usdc_loan.loan_id);
        manager_client.add_interest(&eurc_loan.loan_id);

        // Here borrowed amount should be the same as time has not moved. add_interest() is only called to store the LastUpdate sequence number.
        assert_eq!(usdc_loan.borrowed_amount, 10_000);
        assert_eq!(usdc_loan.health_factor, 10_004_000);

        // Move time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 10_000;
            li.timestamp = 1 + 8_000_000;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        e.register_at(&reflector_addr, oracle::WASM, ());

        manager_client.add_interest(&usdc_loan.loan_id);
        manager_client.add_interest(&eurc_loan.loan_id);

        usdc_loan = manager_client.get_loan(&usdc_loan.loan_id);

        assert_eq!(usdc_loan.borrowed_amount, 10_760);
        assert_eq!(usdc_loan.health_factor, 9_297_397);
        assert_eq!(usdc_loan.collateral_shares, 12_505);

        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 1_000;
        });

        e.register_at(&reflector_addr, oracle::WASM, ());

        manager_client.liquidate(&admin, &usdc_loan.loan_id, &5_000);

        usdc_loan = manager_client.get_loan(&usdc_loan.loan_id);
        assert_eq!(usdc_loan.borrowed_amount, 5_760);
        assert_eq!(usdc_loan.health_factor, 9_729_166);
        assert_eq!(usdc_loan.collateral_shares, 7_005);

        eurc_loan = manager_client.get_loan(&eurc_loan.loan_id);
        assert_eq!(eurc_loan.borrowed_amount, 10_760);
        assert_eq!(eurc_loan.health_factor, 9_297_397);
        assert_eq!(eurc_loan.collateral_shares, 12_505);
    }

    #[test]
    fn test_new_storage_layout() {
        // Test that the new storage layout works correctly
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let TestEnv {
            user,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            xlm_asset_client,
            ..
        } = setup_test_env(&e);

        // Give user more XLM for collateral
        xlm_asset_client.mint(&user, &10_000);

        // Create multiple loans for the same user
        let mut loan1 =
            manager_client.create_loan(&user, &100, &pool_usdc_addr, &1000, &pool_xlm_addr);
        let mut loan2 =
            manager_client.create_loan(&user, &200, &pool_usdc_addr, &2000, &pool_xlm_addr);
        let mut loan3 =
            manager_client.create_loan(&user, &300, &pool_usdc_addr, &3000, &pool_xlm_addr);

        // Verify all loans are stored and retrievable
        let loans = manager_client.get_loans(&user);
        assert_eq!(loans.len(), 3);

        // Verify individual loans can be accessed by loan id
        loan1 = manager_client.get_loan(&loan1.loan_id);
        loan2 = manager_client.get_loan(&loan2.loan_id);
        loan3 = manager_client.get_loan(&loan3.loan_id);

        assert_eq!(loan1.borrowed_amount, 100);
        assert_eq!(loan2.borrowed_amount, 200);
        assert_eq!(loan3.borrowed_amount, 300);
    }

    #[test]
    fn lai_borrower_earns_rewards_after_first_borrow() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 10_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 10_000_001;
        });

        let TestEnv {
            user,
            manager_addr,
            manager_client,
            pool_xlm_addr,
            pool_usdc_addr,
            ..
        } = setup_test_env(&e);

        // Register LAI token and fund the loan manager with 50M LAI
        let lai_admin = Address::generate(&e);
        let lai_addr = e.register_stellar_asset_contract_v2(lai_admin).address();
        let lai_asset_client = StellarAssetClient::new(&e, &lai_addr);
        let lai_token_client = TokenClient::new(&e, &lai_addr);
        lai_asset_client.mint(&manager_addr, &(50_000_000i128 * 10_000_000i128));

        // Initialize LAI distribution starting at current ledger
        manager_client.initialize_lai_distribution(&lai_addr, &100_000u32);

        // First ever borrow: total_liabilities == 0 before this call.
        // BUG: checkpoint_borrower_internal sets last_known_total = 0 (the old total)
        // and never updates it to the post-borrow total, so advance_accumulator always
        // skips reward accumulation, leaving this borrower with 0 LAI forever.
        manager_client.create_loan(&user, &100, &pool_usdc_addr, &1_000, &pool_xlm_addr);

        // Advance ~100k ledgers so rewards should have accumulated
        e.ledger().with_mut(|li| {
            li.sequence_number = 200_000;
        });

        let rewards = manager_client.claim_lai_rewards(&user);
        assert!(
            rewards > 0,
            "First borrower should earn LAI after 100k ledgers, got {rewards}"
        );
        assert_eq!(lai_token_client.balance(&user), rewards);
    }

    #[test]
    fn lai_insurer_earns_rewards_after_first_deposit() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 10_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 10_000_001;
        });

        let TestEnv {
            user,
            manager_addr,
            manager_client,
            pool_usdc_addr,
            ..
        } = setup_test_env(&e);

        // Register LAI token and fund the loan manager with 50M LAI
        let lai_admin = Address::generate(&e);
        let lai_addr = e.register_stellar_asset_contract_v2(lai_admin).address();
        let lai_asset_client = StellarAssetClient::new(&e, &lai_addr);
        let lai_token_client = TokenClient::new(&e, &lai_addr);
        lai_asset_client.mint(&manager_addr, &(50_000_000i128 * 10_000_000i128));

        // Initialize LAI distribution starting at current ledger
        manager_client.initialize_lai_distribution(&lai_addr, &100_000u32);

        // Register an insurance pool for the USDC pool
        let ins_pool_addr = Address::generate(&e);
        manager_client.set_insurance_pool(&pool_usdc_addr, &ins_pool_addr);

        // First ever insurer: total_shares == 0 before this deposit.
        // BUG: checkpoint_insurer_internal sets last_known_total = 0 (the old total)
        // and never updates it to the post-deposit total, so advance_accumulator always
        // skips reward accumulation, leaving this insurer with 0 LAI forever.
        // Simulate a user depositing 500 shares (old=0, new=500, total=0)
        manager_client.checkpoint_insurer_reward(&ins_pool_addr, &user, &0i128, &500i128, &0i128);

        // Advance ~100k ledgers so rewards should have accumulated
        e.ledger().with_mut(|li| {
            li.sequence_number = 200_000;
        });

        let rewards = manager_client.claim_lai_rewards(&user);
        assert!(
            rewards > 0,
            "First insurer should earn LAI after 100k ledgers, got {rewards}"
        );
        assert_eq!(lai_token_client.balance(&user), rewards);
    }

    /* Test setup helpers */
    struct TestEnv<'a> {
        admin: Address,
        user: Address,
        manager_addr: Address,
        manager_client: LoanManagerClient<'a>,
        xlm_asset_client: StellarAssetClient<'a>,
        xlm_token_client: TokenClient<'a>,
        usdc_asset_client: StellarAssetClient<'a>,
        usdc_token_client: TokenClient<'a>,
        pool_xlm_addr: Address,
        pool_xlm_client: loan_pool::Client<'a>,
        pool_usdc_addr: Address,
        reflector_addr: Address,
        pool_usdc_client: loan_pool::Client<'a>,
        eurc_asset_client: StellarAssetClient<'a>,
        eurc_token_client: TokenClient<'a>,
        pool_eurc_addr: Address,
        pool_eurc_client: loan_pool::Client<'a>,
    }

    fn setup_test_env(e: &Env) -> TestEnv<'_> {
        let admin = Address::generate(e);
        let admin2 = Address::generate(e);
        let admin3 = Address::generate(e);
        let user = Address::generate(e);
        let oracle = Address::generate(e);

        // loan manager
        let manager_addr = e.register(LoanManager, ());
        let manager_client = LoanManagerClient::new(e, &manager_addr);
        manager_client.initialize(&admin, &oracle);

        // XLM asset
        let xlm_ticker = Symbol::new(e, "XLM");
        let xlm_addr = e
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let xlm_asset_client = StellarAssetClient::new(e, &xlm_addr);
        let xlm_token_client = TokenClient::new(e, &xlm_addr);

        // XLM pool
        let pool_xlm_addr = setup_test_pool(e, &manager_client, &xlm_ticker, &xlm_addr);
        let pool_xlm_client = loan_pool::Client::new(e, &pool_xlm_addr);

        // USDC asset
        let usdc_ticker = Symbol::new(e, "USDC");
        let usdc_addr = e
            .register_stellar_asset_contract_v2(admin2.clone())
            .address();
        let usdc_asset_client = StellarAssetClient::new(e, &usdc_addr);
        let usdc_token_client = TokenClient::new(e, &usdc_addr);

        // USDC pool
        let pool_usdc_addr = setup_test_pool(e, &manager_client, &usdc_ticker, &usdc_addr);
        let pool_usdc_client = loan_pool::Client::new(e, &pool_usdc_addr);

        // EURC asset
        let eurc_ticker = Symbol::new(e, "EURC");
        let eurc_addr = e
            .register_stellar_asset_contract_v2(admin3.clone())
            .address();
        let eurc_asset_client = StellarAssetClient::new(e, &eurc_addr);
        let eurc_token_client = TokenClient::new(e, &eurc_addr);

        // EURC pool
        let pool_eurc_addr = setup_test_pool(e, &manager_client, &eurc_ticker, &eurc_addr);
        let pool_eurc_client = loan_pool::Client::new(e, &pool_eurc_addr);

        // Mint the admin and the user some coins
        xlm_asset_client.mint(&user, &1_000);
        usdc_asset_client.mint(&admin, &1_000_000);
        eurc_asset_client.mint(&admin, &1_000_000);

        // Setup mock price oracle
        let reflector_addr = manager_client.get_oracle();
        e.register_at(&reflector_addr, oracle::WASM, ());

        // Deposit some of the admin's tokens for borrowing.
        pool_usdc_client.deposit(&admin, &1_000);
        pool_eurc_client.deposit(&admin, &1_000);

        // No insurance pool is set up in the test environment, so transition the pools to
        // Healthy manually by supplying a token count that satisfies the ≥10% threshold.
        pool_usdc_client.update_status(&1_000);
        pool_eurc_client.update_status(&1_000);

        TestEnv {
            admin,
            user,
            manager_addr,
            manager_client,
            xlm_asset_client,
            xlm_token_client,
            usdc_asset_client,
            usdc_token_client,
            pool_xlm_addr,
            pool_xlm_client,
            pool_usdc_addr,
            reflector_addr,
            pool_usdc_client,
            eurc_asset_client,
            eurc_token_client,
            pool_eurc_addr,
            pool_eurc_client,
        }
    }

    fn setup_test_pool(
        e: &Env,
        manager_client: &LoanManagerClient,
        ticker: &Symbol,
        token_address: &Address,
    ) -> Address {
        use soroban_sdk::String;

        const LIQUIDATION_THRESHOLD: i128 = 8_000_000; // 80%
        let wasm_hash = e.deployer().upload_contract_wasm(loan_pool::WASM);
        let xdr_bytes = token_address.clone().to_xdr(e);
        let salt = e.crypto().sha256(&xdr_bytes).to_bytes();

        // Precompute pool address so we can set it as admin of the share token
        let manager_addr = manager_client.address.clone();
        let pool_addr = e
            .deployer()
            .with_address(manager_addr, salt.clone())
            .deployed_address();

        // Register share token with pool as admin (pool must be able to mint/burn)
        let share_token_addr = e.register(
            share_token_wasm::WASM,
            (
                &pool_addr,
                &7u32,
                &String::from_str(e, "lToken"),
                &String::from_str(e, "lT"),
            ),
        );

        manager_client.deploy_pool(
            &wasm_hash,
            &salt,
            token_address,
            ticker,
            &LIQUIDATION_THRESHOLD,
            &share_token_addr,
        )
    }
}
