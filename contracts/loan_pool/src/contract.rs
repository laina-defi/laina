use crate::dto::PoolState;
use crate::error::LoanPoolError;
use crate::interest::{self, get_interest};
use crate::storage::{Currency, PoolStatus, Positions};
use crate::{positions, storage};

use soroban_sdk::{contract, contractimpl, contractmeta, token, Address, BytesN, Env};

// Metadata that is added on to the WASM custom section
contractmeta!(
    key = "Desc",
    val = "Lending pool with variable interest rate."
);

mod share_token {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/token.wasm");
}

mod insurance_pool {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/insurance_pool.wasm");
}

#[contract]
struct LoanPoolContract;

#[allow(dead_code)]
#[contractimpl]
impl LoanPoolContract {
    /// Sets the currency of the pool and initializes its balance.
    pub fn initialize(
        e: Env,
        loan_manager_addr: Address,
        currency: Currency,
        liquidation_threshold: i128,
        token_contract_address: Address,
    ) {
        storage::write_loan_manager_addr(&e, loan_manager_addr);
        storage::write_currency(&e, currency);
        storage::write_liquidation_threshold(&e, liquidation_threshold);
        storage::write_total_shares(&e, 0);
        storage::write_total_balance(&e, 0);
        storage::write_available_balance(&e, 0);
        storage::write_accrual(&e, 10_000_000); // Default initial accrual value.
        storage::write_accrual_last_updated(&e, e.ledger().timestamp());
        storage::change_interest_rate_multiplier(&e, 1); // Temporary parameter
        storage::change_pool_status(&e, PoolStatus::Caution);
        storage::write_token_contract_address(&e, token_contract_address);
    }

    pub fn upgrade(e: Env, new_wasm_hash: BytesN<32>) -> Result<(), LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        e.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    pub fn change_interest_rate_multiplier(e: Env, multiplier: i128) -> Result<(), LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        storage::change_interest_rate_multiplier(&e, multiplier);
        Ok(())
    }

    /// Deposits token. Also, mints pool shares for the "user" Identifier.
    pub fn deposit(e: Env, user: Address, amount: i128) -> Result<i128, LoanPoolError> {
        user.require_auth();

        let pool_status = storage::read_pool_status(&e)?;
        if pool_status == PoolStatus::Restricted || pool_status == PoolStatus::Frozen {
            return Err(LoanPoolError::WrongStatus);
        }

        if amount <= 0 {
            Err(LoanPoolError::NegativeDeposit)
        } else {
            Self::add_interest_to_accrual(e.clone())?;

            let token_address = storage::read_currency(&e)?.token_address;

            let client = token::Client::new(&e, &token_address);
            client.transfer(&user, e.current_contract_address(), &amount);

            let current_shares = Self::get_total_balance_shares(e.clone())?;
            let current_contract_balance = Self::get_contract_balance(e.clone())?;

            let shares_issued = if current_contract_balance == 0 {
                amount
            } else {
                current_shares
                    .checked_mul(amount)
                    .ok_or(LoanPoolError::OverOrUnderFlow)?
                    .checked_div(current_contract_balance)
                    .ok_or(LoanPoolError::OverOrUnderFlow)?
            };

            let share_token_client =
                share_token::Client::new(&e, &storage::read_token_contract_address(&e)?);

            share_token_client.mint(&user, &shares_issued);

            storage::adjust_available_balance(&e, amount)?;
            storage::adjust_total_shares(&e, shares_issued)?;
            storage::adjust_total_balance(&e, amount)?;

            Self::check_and_update_status(&e);
            Ok(amount)
        }
    }

    /// Transfers share tokens back, burns them and gives corresponding amount of tokens back to user. Returns amount of tokens withdrawn
    pub fn withdraw(e: Env, user: Address, amount: i128) -> Result<PoolState, LoanPoolError> {
        user.require_auth();

        let pool_status = storage::read_pool_status(&e)?;
        if pool_status == PoolStatus::Frozen {
            return Err(LoanPoolError::WrongStatus);
        }

        Self::withdraw_internal(e, user, amount)
    }

    fn withdraw_internal(e: Env, user: Address, amount: i128) -> Result<PoolState, LoanPoolError> {
        Self::add_interest_to_accrual(e.clone())?;

        // Get user's receivable shares from the share token (single source of truth)
        let share_token_client =
            share_token::Client::new(&e, &storage::read_token_contract_address(&e)?);
        let receivable_shares = share_token_client.balance(&user);

        let available_balance_tokens = Self::get_available_balance(e.clone())?;
        if amount > available_balance_tokens {
            return Err(LoanPoolError::WithdrawOverBalance);
        }
        let total_balance_shares = Self::get_total_balance_shares(e.clone())?;
        let total_balance_tokens = Self::get_contract_balance(e.clone())?;
        let shares_to_decrease = amount
            .checked_mul(total_balance_shares)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(total_balance_tokens)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        // Check that user is not trying to move more than receivables (TODO: also include collateral?)
        if shares_to_decrease > receivable_shares {
            return Err(LoanPoolError::WithdrawIsNegative);
        }

        let new_available_balance_tokens = storage::adjust_available_balance(
            &e,
            amount.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        let new_total_balance_tokens = storage::adjust_total_balance(
            &e,
            amount.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        let new_total_balance_shares = storage::adjust_total_shares(
            &e,
            shares_to_decrease
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;

        // Transfer tokens from the pool to the user
        let token_address = &storage::read_currency(&e)?.token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&e.current_contract_address(), &user, &amount);

        let share_token_client =
            share_token::Client::new(&e, &storage::read_token_contract_address(&e)?);

        share_token_client.burn(&user, &shares_to_decrease);

        let new_annual_interest_rate = Self::get_interest(e.clone())?;

        Self::check_and_update_status(&e);

        let pool_state = PoolState {
            total_balance_tokens: new_total_balance_tokens,
            available_balance_tokens: new_available_balance_tokens,
            total_liabilities_tokens: storage::read_total_liabilities(&e),
            total_balance_shares: new_total_balance_shares,
            annual_interest_rate: new_annual_interest_rate,
            pool_status: storage::read_pool_status(&e)?,
        };
        Ok(pool_state)
    }

    /// Borrow tokens from the pool
    pub fn borrow(e: Env, user: Address, amount: i128) -> Result<i128, LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();
        user.require_auth();

        let pool_status = storage::read_pool_status(&e)?;
        if pool_status != PoolStatus::Healthy {
            return Err(LoanPoolError::WrongStatus);
        }

        Self::add_interest_to_accrual(e.clone())?;

        let balance = storage::read_available_balance(&e)?;
        assert!(
            amount < balance,
            "Borrowed amount has to be less than available balance!"
        );

        storage::adjust_available_balance(
            &e,
            amount.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;

        positions::increase_positions(&e, user.clone(), amount, 0)?;
        storage::adjust_total_liabilities(&e, amount)?;

        let token_address = &storage::read_currency(&e)?.token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&e.current_contract_address(), &user, &amount);

        Ok(amount)
    }

    /// Deposit tokens to the pool to be used as collateral.
    /// Tokens are added to pool liquidity and earn interest via the share mechanism.
    /// Returns the number of collateral shares issued.
    pub fn deposit_collateral(e: Env, user: Address, amount: i128) -> Result<i128, LoanPoolError> {
        user.require_auth();
        assert!(amount > 0, "Amount must be positive!");

        Self::add_interest_to_accrual(e.clone())?;

        let total_shares = Self::get_total_balance_shares(e.clone())?;
        let total_tokens = Self::get_contract_balance(e.clone())?;

        let shares_issued = if total_tokens == 0 {
            amount
        } else {
            total_shares
                .checked_mul(amount)
                .ok_or(LoanPoolError::OverOrUnderFlow)?
                .checked_div(total_tokens)
                .ok_or(LoanPoolError::OverOrUnderFlow)?
        };

        let token_address = &storage::read_currency(&e)?.token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&user, e.current_contract_address(), &amount);

        positions::increase_positions(&e, user.clone(), 0, shares_issued)?;
        storage::adjust_available_balance(&e, amount)?;
        storage::adjust_total_balance(&e, amount)?;
        storage::adjust_total_shares(&e, shares_issued)?;

        Self::check_and_update_status(&e);
        Ok(shares_issued)
    }

    /// Withdraw collateral from the pool. Takes shares and returns the current token value
    /// (including accrued interest). Only callable by the loan manager.
    pub fn withdraw_collateral(e: Env, user: Address, shares: i128) -> Result<i128, LoanPoolError> {
        user.require_auth();
        Self::add_interest_to_accrual(e.clone())?;

        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();
        assert!(shares > 0, "Shares must be positive!");

        let total_shares = storage::read_total_shares(&e)?;
        let total_tokens = storage::read_total_balance(&e)?;
        let tokens = shares
            .checked_mul(total_tokens)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(total_shares)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        positions::decrease_positions(&e, user.clone(), 0, shares)?;
        storage::adjust_available_balance(
            &e,
            tokens.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        storage::adjust_total_balance(
            &e,
            tokens.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        storage::adjust_total_shares(
            &e,
            shares.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;

        let token_address = &storage::read_currency(&e)?.token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&e.current_contract_address(), &user, &tokens);

        Ok(tokens)
    }

    /// Convert a share amount to its current token value.
    pub fn shares_to_tokens(e: Env, shares: i128) -> Result<i128, LoanPoolError> {
        let total_shares = storage::read_total_shares(&e)?;
        let total_tokens = storage::read_total_balance(&e)?;
        if total_shares == 0 {
            return Ok(0);
        }
        shares
            .checked_mul(total_tokens)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(total_shares)
            .ok_or(LoanPoolError::OverOrUnderFlow)
    }

    /// Convert a token amount to the equivalent number of shares at the current rate.
    pub fn tokens_to_shares(e: Env, tokens: i128) -> Result<i128, LoanPoolError> {
        let total_shares = storage::read_total_shares(&e)?;
        let total_tokens = storage::read_total_balance(&e)?;
        if total_tokens == 0 {
            return Ok(tokens);
        }
        tokens
            .checked_mul(total_shares)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(total_tokens)
            .ok_or(LoanPoolError::OverOrUnderFlow)
    }

    pub fn add_interest_to_accrual(e: Env) -> Result<(), LoanPoolError> {
        const DECIMAL: i128 = 10000000;
        const SECONDS_IN_YEAR: u64 = 31_556_926;

        let current_timestamp = e.ledger().timestamp();
        let accrual = storage::read_accrual(&e)?;
        let accrual_last_update = storage::read_accrual_last_updated(&e)?;
        let ledgers_since_update = current_timestamp
            .checked_sub(accrual_last_update)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;
        let ledger_ratio: i128 = (i128::from(ledgers_since_update))
            .checked_mul(DECIMAL)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(i128::from(SECONDS_IN_YEAR))
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let interest_rate: i128 = get_interest(e.clone())?;
        let interest_amount_in_year: i128 = accrual
            .checked_mul(interest_rate)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(DECIMAL)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;
        let interest_since_update: i128 = interest_amount_in_year
            .checked_mul(ledger_ratio)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(DECIMAL)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;
        let new_accrual: i128 = accrual
            .checked_add(interest_since_update)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        storage::write_accrual_last_updated(&e, current_timestamp);
        storage::write_accrual(&e, new_accrual);
        Ok(())
    }

    pub fn get_accrual(e: &Env) -> Result<i128, LoanPoolError> {
        storage::read_accrual(e)
    }

    pub fn get_collateral_factor(e: &Env) -> Result<i128, LoanPoolError> {
        storage::read_collateral_factor(e)
    }

    pub fn get_interest_rate_multiplier(e: &Env) -> Result<i128, LoanPoolError> {
        storage::read_interest_rate_multiplier(e)
    }

    /// Get user's positions. receivable_shares is sourced from the share token (single source of truth).
    pub fn get_user_positions(e: Env, user: Address) -> Positions {
        let stored = storage::read_stored_positions(&e, &user);
        let share_token_client = share_token::Client::new(
            &e,
            &storage::read_token_contract_address(&e)
                .unwrap_or_else(|_| panic!("token contract address not set")),
        );
        Positions {
            receivable_shares: share_token_client.balance(&user),
            liabilities: stored.liabilities,
            collateral_shares: stored.collateral_shares,
        }
    }

    /// Get contract data entries
    pub fn get_contract_balance(e: Env) -> Result<i128, LoanPoolError> {
        storage::read_total_balance(&e)
    }

    pub fn get_total_balance_shares(e: Env) -> Result<i128, LoanPoolError> {
        storage::read_total_shares(&e)
    }

    pub fn get_available_balance(e: Env) -> Result<i128, LoanPoolError> {
        storage::read_available_balance(&e)
    }

    pub fn get_currency(e: Env) -> Result<Currency, LoanPoolError> {
        storage::read_currency(&e)
    }

    pub fn get_interest(e: Env) -> Result<i128, LoanPoolError> {
        interest::get_interest(e)
    }

    pub fn get_pool_state(e: Env) -> Result<PoolState, LoanPoolError> {
        Ok(PoolState {
            total_balance_tokens: storage::read_total_balance(&e)?,
            available_balance_tokens: storage::read_available_balance(&e)?,
            total_liabilities_tokens: storage::read_total_liabilities(&e),
            total_balance_shares: storage::read_total_shares(&e)?,
            annual_interest_rate: interest::get_interest(e.clone())?,
            pool_status: storage::read_pool_status(&e)?,
        })
    }

    /// Recompute and persist pool status based on the insurance-to-loan-pool token ratio.
    /// Transitions between Healthy and Caution only; never overrides Frozen or Restricted.
    /// Called by the insurance pool after deposits/withdrawals, passing its current token total
    /// directly to avoid a re-entrant call back into the insurance pool.
    pub fn update_status(e: Env, insurance_tokens: i128) {
        Self::apply_status_logic(&e, insurance_tokens);
    }

    /// Reads the insurance pool state and applies status logic. Only safe to call when the
    /// insurance pool is NOT already on the call stack (i.e. from loan pool's own deposit/withdraw).
    fn check_and_update_status(e: &Env) {
        let insurance_pool_addr = match storage::read_insurance_pool_address(e) {
            Ok(addr) => addr,
            Err(_) => return, // No insurance pool registered yet — stay Caution.
        };
        let ins_tokens = insurance_pool::Client::new(e, &insurance_pool_addr)
            .get_pool_state()
            .total_tokens;
        Self::apply_status_logic(e, ins_tokens);
    }

    fn apply_status_logic(e: &Env, ins_tokens: i128) {
        let current = match storage::read_pool_status(e) {
            Ok(s) => s,
            Err(_) => return,
        };
        // Never automatically override Frozen or Restricted (manual admin states).
        if current == PoolStatus::Frozen || current == PoolStatus::Restricted {
            return;
        }

        let loan_tokens = match storage::read_total_balance(e) {
            Ok(t) => t,
            Err(_) => return,
        };

        let new_status = if ins_tokens > 0 && ins_tokens.saturating_mul(10) >= loan_tokens {
            PoolStatus::Healthy
        } else {
            PoolStatus::Caution
        };

        if new_status != current {
            storage::change_pool_status(e, new_status);
        }
    }

    pub fn increase_liabilities(e: Env, user: Address, amount: i128) -> Result<(), LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        positions::increase_positions(&e, user.clone(), amount, 0)?;
        storage::adjust_total_liabilities(&e, amount)?;
        Ok(())
    }

    pub fn repay(
        e: Env,
        user: Address,
        amount: i128,
        unpaid_interest: i128,
    ) -> Result<(), LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        let pool_status = storage::read_pool_status(&e)?;
        if pool_status == PoolStatus::Frozen {
            return Err(LoanPoolError::WrongStatus);
        }

        Self::add_interest_to_accrual(e.clone())?;

        // Split the repayment into interest and principal portions
        let interest_paid = if amount < unpaid_interest {
            amount
        } else {
            unpaid_interest
        };
        let principal_paid = amount
            .checked_sub(interest_paid)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        // Admin fee applies only to the interest portion (10%)
        let amount_to_admin = interest_paid / 10;

        // Net amount that stays in the pool contract
        let amount_to_storage = amount
            .checked_sub(amount_to_admin)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let client = token::Client::new(&e, &storage::read_currency(&e)?.token_address);
        client.transfer(&user, e.current_contract_address(), &amount_to_storage);
        client.transfer(&user, &loan_manager_addr, &amount_to_admin);

        // Get current user liabilities to ensure we don't decrease by more than they have
        let current_liabilities = storage::read_stored_positions(&e, &user).liabilities;

        // Only decrease liabilities by the minimum of principal_paid and current_liabilities
        let liabilities_to_decrease = if principal_paid > current_liabilities {
            current_liabilities
        } else {
            principal_paid
        };

        positions::decrease_positions(&e, user, liabilities_to_decrease, 0)?;
        storage::adjust_total_liabilities(
            &e,
            liabilities_to_decrease
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;

        // All net paid funds (principal + interest - admin) increase available liquidity
        storage::adjust_available_balance(&e, amount - amount_to_admin)?;

        // Only the interest portion (net of admin) increases the pool's total balance
        storage::adjust_total_balance(&e, interest_paid - amount_to_admin)?;

        let net_interest = interest_paid - amount_to_admin;
        Self::boost_insurance_pool(&e, net_interest)?;
        Ok(())
    }

    pub fn repay_and_close(
        e: Env,
        user: Address,
        borrowed_amount: i128,
        max_allowed_amount: i128,
        unpaid_interest: i128,
    ) -> Result<(), LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        let pool_status = storage::read_pool_status(&e)?;
        if pool_status == PoolStatus::Frozen {
            return Err(LoanPoolError::WrongStatus);
        }

        Self::add_interest_to_accrual(e.clone())?;

        let amount_to_admin = if borrowed_amount < unpaid_interest {
            borrowed_amount / 10
        } else {
            unpaid_interest / 10
        };

        let amount_to_user = max_allowed_amount
            .checked_sub(borrowed_amount)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let client = token::Client::new(&e, &storage::read_currency(&e)?.token_address);
        client.transfer(&user, e.current_contract_address(), &max_allowed_amount);
        client.transfer(
            &e.current_contract_address(),
            &loan_manager_addr,
            &amount_to_admin,
        );
        client.transfer(&e.current_contract_address(), &user, &amount_to_user);

        let user_liabilities = storage::read_stored_positions(&e, &user).liabilities;
        positions::decrease_positions(&e, user, user_liabilities, 0)?;
        storage::adjust_total_liabilities(
            &e,
            user_liabilities
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        storage::adjust_available_balance(&e, borrowed_amount - amount_to_admin)?;
        storage::adjust_total_balance(&e, unpaid_interest - amount_to_admin)?;

        let net_interest = unpaid_interest - amount_to_admin;
        Self::boost_insurance_pool(&e, net_interest)?;
        Ok(())
    }

    pub fn liquidate(
        e: Env,
        user: Address,
        amount: i128,
        unpaid_interest: i128,
        loan_owner: Address,
    ) -> Result<(), LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();
        user.require_auth();

        let pool_status = storage::read_pool_status(&e)?;
        if pool_status == PoolStatus::Frozen {
            return Err(LoanPoolError::WrongStatus);
        }

        Self::add_interest_to_accrual(e.clone())?;

        let amount_to_admin = if amount < unpaid_interest {
            amount / 10
        } else {
            unpaid_interest / 10
        };

        let amount_to_storage = amount
            .checked_sub(amount_to_admin)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let client = token::Client::new(&e, &storage::read_currency(&e)?.token_address);
        client.transfer(&user, e.current_contract_address(), &amount_to_storage);
        client.transfer(&user, &loan_manager_addr, &amount_to_admin);

        positions::decrease_positions(&e, loan_owner, amount, 0)?;
        storage::adjust_total_liabilities(
            &e,
            amount.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        storage::adjust_available_balance(&e, amount_to_storage)?;
        Ok(())
    }

    /// Transfer collateral to a liquidator. Takes a token amount, converts to shares,
    /// and transfers the actual tokens. Returns the number of shares removed.
    pub fn liquidate_transfer_collateral(
        e: Env,
        user: Address,
        amount_collateral_tokens: i128,
        loan_owner: Address,
    ) -> Result<i128, LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        let total_shares = storage::read_total_shares(&e)?;
        let total_tokens = storage::read_total_balance(&e)?;
        let shares_to_remove = amount_collateral_tokens
            .checked_mul(total_shares)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(total_tokens)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        positions::decrease_positions(&e, loan_owner, 0, shares_to_remove)?;
        storage::adjust_available_balance(
            &e,
            amount_collateral_tokens
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        storage::adjust_total_balance(
            &e,
            amount_collateral_tokens
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        storage::adjust_total_shares(
            &e,
            shares_to_remove
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;

        let client = token::Client::new(&e, &storage::read_currency(&e)?.token_address);
        client.transfer(
            &e.current_contract_address(),
            &user,
            &amount_collateral_tokens,
        );

        Ok(shares_to_remove)
    }

    pub fn claim_bad_debt_payment(
        e: Env,
        user: Address,
        claim_amount: i128,
        unpaid_interest: i128,
        max_allowed_amount: i128,
        loan_owner: Address,
    ) -> Result<(), LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        let pool_status = storage::read_pool_status(&e)?;
        if pool_status == PoolStatus::Frozen {
            return Err(LoanPoolError::WrongStatus);
        }

        Self::add_interest_to_accrual(e.clone())?;

        let amount_to_admin = if claim_amount < unpaid_interest {
            claim_amount / 10
        } else {
            unpaid_interest / 10
        };

        let amount_to_user = max_allowed_amount
            .checked_sub(claim_amount)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let client = token::Client::new(&e, &storage::read_currency(&e)?.token_address);
        client.transfer(&user, e.current_contract_address(), &max_allowed_amount);
        client.transfer(
            &e.current_contract_address(),
            &loan_manager_addr,
            &amount_to_admin,
        );
        client.transfer(&e.current_contract_address(), &user, &amount_to_user);

        let loan_owner_liabilities = storage::read_stored_positions(&e, &loan_owner).liabilities;
        positions::decrease_positions(&e, loan_owner, loan_owner_liabilities, 0)?;
        storage::adjust_total_liabilities(
            &e,
            loan_owner_liabilities
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        storage::adjust_available_balance(&e, claim_amount - amount_to_admin)?;
        storage::adjust_total_balance(&e, unpaid_interest - amount_to_admin)?;

        let net_interest = unpaid_interest - amount_to_admin;
        Self::boost_insurance_pool(&e, net_interest)?;
        Self::check_and_update_status(&e);
        Ok(())
    }

    /// Set the insurance pool address. Only callable by the loan manager.
    pub fn set_insurance_pool(e: Env, insurance_pool_addr: Address) -> Result<(), LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();
        storage::write_insurance_pool_address(&e, insurance_pool_addr);
        Ok(())
    }

    /// Mints l-tokens equivalent to half of `net_interest` directly to the insurance pool,
    /// then notifies it to update its internal token accounting. This gives insurers a
    /// higher effective yield than plain depositors, compensating them for bad-debt risk.
    /// Skipped silently if no insurance pool is configured or if it has no active insurers.
    fn boost_insurance_pool(e: &Env, net_interest: i128) -> Result<(), LoanPoolError> {
        if net_interest <= 1 {
            return Ok(());
        }

        let insurance_pool_addr = match storage::read_insurance_pool_address(e) {
            Ok(addr) => addr,
            Err(_) => return Ok(()),
        };

        let insurance_client = insurance_pool::Client::new(e, &insurance_pool_addr);
        let pool_state = insurance_client.get_pool_state();
        if pool_state.total_shares == 0 {
            return Ok(());
        }

        let total_shares = storage::read_total_shares(e)?;
        let total_balance = storage::read_total_balance(e)?;

        if total_balance == 0 {
            return Ok(());
        }

        // boost ≈ (net_interest / 2) × total_shares / total_balance
        let boost_shares = (net_interest / 2)
            .checked_mul(total_shares)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(total_balance)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        if boost_shares == 0 {
            return Ok(());
        }

        let share_token_client =
            share_token::Client::new(e, &storage::read_token_contract_address(e)?);
        share_token_client.mint(&insurance_pool_addr, &boost_shares);
        storage::adjust_total_shares(e, boost_shares)?;
        insurance_client.receive_interest_boost(&boost_shares);

        Ok(())
    }

    /// Write off bad debt by burning equivalent lXLM from the insurance pool.
    /// This preserves the token-to-share ratio for regular depositors.
    /// Only callable by the loan manager.
    pub fn write_off_bad_debt(
        e: Env,
        borrower: Address,
        bad_debt_tokens: i128,
    ) -> Result<(), LoanPoolError> {
        let loan_manager_addr = storage::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        if bad_debt_tokens <= 0 {
            return Err(LoanPoolError::BadDebtWrite);
        }

        Self::add_interest_to_accrual(e.clone())?;

        let total_shares = storage::read_total_shares(&e)?;
        let total_tokens = storage::read_total_balance(&e)?;

        let shares_to_burn = bad_debt_tokens
            .checked_mul(total_shares)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(total_tokens)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        if shares_to_burn > 0 {
            let insurance_pool_addr = storage::read_insurance_pool_address(&e)?;
            let insurance_client = insurance_pool::Client::new(&e, &insurance_pool_addr);
            insurance_client
                .try_cover_bad_debt(&shares_to_burn)
                .map_err(|_| LoanPoolError::InsufficientInsuranceCoverage)?
                .map_err(|_| LoanPoolError::InsufficientInsuranceCoverage)?;
        }

        let current_liabilities = storage::read_stored_positions(&e, &borrower).liabilities;
        let liabilities_to_decrease = bad_debt_tokens.min(current_liabilities);
        positions::decrease_positions(&e, borrower, liabilities_to_decrease, 0)?;
        storage::adjust_total_liabilities(
            &e,
            liabilities_to_decrease
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;

        storage::adjust_total_balance(
            &e,
            bad_debt_tokens
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        storage::adjust_total_shares(
            &e,
            shares_to_burn
                .checked_neg()
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;

        Self::check_and_update_status(&e);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::{Client as TokenClient, StellarAssetClient},
        Env, String, Symbol,
    };

    mod share_token_wasm {
        soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/token.wasm");
    }

    // Minimal stub that satisfies the get_pool_state cross-contract call made by
    // check_and_update_status, without requiring the full insurance_pool WASM.
    // Always returns enough tokens to satisfy the ≥10% threshold.
    mod mock_insurance_pool {
        use soroban_sdk::{contract, contractimpl, contracttype, Env};

        #[contracttype]
        pub struct InsurancePoolState {
            pub total_tokens: i128,
            pub total_shares: i128,
        }

        #[contract]
        pub struct MockInsurancePool;

        #[contractimpl]
        impl MockInsurancePool {
            pub fn get_pool_state(_e: Env) -> InsurancePoolState {
                InsurancePoolState {
                    total_tokens: 1_000_000_000,
                    total_shares: 1_000_000_000,
                }
            }
        }
    }

    const TEST_LIQUIDATION_THRESHOLD: i128 = 8_000_000;

    /// Register a share token contract with the given pool address as admin.
    fn create_share_token(e: &Env, pool_addr: &Address) -> Address {
        e.register(
            share_token_wasm::WASM,
            (
                &pool_addr.clone(),
                &7u32,
                &String::from_str(e, "lToken"),
                &String::from_str(e, "lT"),
            ),
        )
    }

    #[test]
    fn initialize() {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );
    }

    #[test]
    fn deposit() {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);
        let amount: i128 = 100;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );

        let result: i128 = contract_client.deposit(&user, &amount);

        assert_eq!(result, amount);
    }

    #[test]
    fn borrow() {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);

        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let asset = StellarAssetClient::new(&e, &token.address());

        let token_client = TokenClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);
        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );

        // Register mock insurance pool so check_and_update_status sets the pool to Healthy.
        let mock_ins_pool = e.register(mock_insurance_pool::MockInsurancePool, ());
        contract_client.set_insurance_pool(&mock_ins_pool);

        // Deposit funds for the borrower to loan.
        let depositer = Address::generate(&e);
        asset.mint(&depositer, &100);
        contract_client.deposit(&depositer, &100);

        // Borrow some of those funds
        let borrower = Address::generate(&e);
        contract_client.borrow(&borrower, &50);

        // Did the funds move?
        assert_eq!(token_client.balance(&depositer), 0);
        assert_eq!(token_client.balance(&borrower), 50);
    }

    #[test]
    fn withdraw() {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);
        let amount: i128 = 100;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );

        let result: i128 = contract_client.deposit(&user, &amount);

        assert_eq!(result, amount);

        contract_client.withdraw(&user, &amount);
    }

    #[test]
    fn repay_and_close() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&e);
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &2000);

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );

        let borrowed_amount = 1000_i128;
        let max_allowed_amount = 1050_i128;
        let unpaid_interest = 20_i128;
        contract_client.repay_and_close(
            &user,
            &borrowed_amount,
            &max_allowed_amount,
            &unpaid_interest,
        );
    }

    #[test]
    #[should_panic]
    fn deposit_more_than_account_balance() {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);
        let amount: i128 = 2000;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );

        contract_client.deposit(&user, &amount);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #11)")]
    fn withdraw_more_than_balance() {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);
        let amount: i128 = 100;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );

        let result: i128 = contract_client.deposit(&user, &amount);

        assert_eq!(result, amount);

        contract_client.withdraw(&user, &(amount * 2));
    }
    #[test]
    #[should_panic]
    fn withdraw_more_than_available_balance() {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);

        let user2 = Address::generate(&e);
        stellar_asset.mint(&user2, &1000);

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);
        let amount: i128 = 100;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );

        let result: i128 = contract_client.deposit(&user, &amount);

        assert_eq!(result, amount);

        contract_client.borrow(&user2, &500);

        let withdraw_result = contract_client.withdraw(&user, &amount);

        assert_eq!(withdraw_result, contract_client.get_pool_state());
    }

    #[test]
    fn add_accrual_full_usage() {
        let e = Env::default();
        e.mock_all_auths();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 10_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 1_000_001;
        });

        let admin = Address::generate(&e);
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);

        let user2 = Address::generate(&e);
        stellar_asset.mint(&user2, &1000);

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);
        let amount: i128 = 1000;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );

        // Register mock insurance pool so check_and_update_status sets the pool to Healthy.
        let mock_ins_pool = e.register(mock_insurance_pool::MockInsurancePool, ());
        contract_client.set_insurance_pool(&mock_ins_pool);

        let result: i128 = contract_client.deposit(&user, &amount);

        assert_eq!(result, amount);

        contract_client.borrow(&user2, &999);

        e.ledger().with_mut(|li| {
            li.timestamp = 1 + 31_556_926; // one year in seconds
        });

        contract_client.add_interest_to_accrual();
        // value of 12980000 is expected as usage is 999/1000 and max interest rate is 30%
        // Time in ledgers is shifted by ~one year.
        assert_eq!(12_980_000, contract_client.get_accrual());

        contract_client.add_interest_to_accrual();
        assert_eq!(12_980_000, contract_client.get_accrual());
    }
    #[test]
    fn add_accrual_half_usage() {
        let e = Env::default();
        e.mock_all_auths();
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000;
            li.timestamp = 1;
            li.min_persistent_entry_ttl = 10_000_000;
            li.min_temp_entry_ttl = 1_000_000;
            li.max_entry_ttl = 1_000_001;
        });

        let admin = Address::generate(&e);
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token.address());
        let currency = Currency {
            token_address: token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);

        let user2 = Address::generate(&e);
        stellar_asset.mint(&user2, &1000);

        let contract_id = e.register(LoanPoolContract, ());
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let share_token_addr = create_share_token(&e, &contract_id);
        let amount: i128 = 1000;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
            &share_token_addr,
        );

        // Register mock insurance pool so check_and_update_status sets the pool to Healthy.
        let mock_ins_pool = e.register(mock_insurance_pool::MockInsurancePool, ());
        contract_client.set_insurance_pool(&mock_ins_pool);

        let result: i128 = contract_client.deposit(&user, &amount);

        assert_eq!(result, amount);

        contract_client.borrow(&user2, &500);

        e.ledger().with_mut(|li| {
            li.timestamp = 1 + 31_556_926; // one year in seconds
        });

        contract_client.add_interest_to_accrual();
        assert_eq!(10_644_440, contract_client.get_accrual());
    }
}
