use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

use crate::{error::InsurancePoolError, storage, storage::WithdrawQueueEntry};

mod share_token {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/token.wasm");
}

#[contracttype]
#[derive(Clone)]
pub struct InsurancePoolState {
    pub total_tokens: i128,
    pub total_shares: i128,
}

#[contract]
pub struct InsurancePool;

#[contractimpl]
impl InsurancePool {
    /// Initialize the insurance pool with its paired loan pool and share token.
    pub fn initialize(
        e: Env,
        loan_pool_addr: Address,
        share_token_addr: Address,
    ) -> Result<(), InsurancePoolError> {
        storage::write_loan_pool_address(&e, loan_pool_addr);
        storage::write_share_token_address(&e, share_token_addr);
        Ok(())
    }

    /// Deposit share tokens (lXLM/lUSDC/lEURC) into the insurance pool.
    /// The user receives internal insurance shares proportional to their deposit.
    /// Returns the number of insurance shares issued.
    pub fn deposit(e: Env, user: Address, amount: i128) -> Result<i128, InsurancePoolError> {
        user.require_auth();

        if amount <= 0 {
            return Err(InsurancePoolError::NegativeAmount);
        }

        let share_token_addr = storage::read_share_token_address(&e)?;
        let share_token_client = share_token::Client::new(&e, &share_token_addr);

        let total_tokens = storage::read_total_insurance_tokens(&e);
        let total_shares = storage::read_total_insurance_shares(&e);

        // Calculate insurance shares to issue
        let insurance_shares_issued = if total_tokens == 0 || total_shares == 0 {
            // First deposit: 1:1 ratio
            amount
        } else {
            amount
                .checked_mul(total_shares)
                .ok_or(InsurancePoolError::OverOrUnderFlow)?
                .checked_div(total_tokens)
                .ok_or(InsurancePoolError::OverOrUnderFlow)?
        };

        // Transfer share tokens from user to this contract
        share_token_client.transfer(&user, &e.current_contract_address(), &amount);

        // Update user's insurance positions
        let positions = storage::read_insurance_positions(&e, &user);
        let new_shares = positions
            .insurance_shares
            .checked_add(insurance_shares_issued)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        storage::write_insurance_positions(&e, user, new_shares);

        // Update totals
        let new_total_tokens = total_tokens
            .checked_add(amount)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        let new_total_shares = total_shares
            .checked_add(insurance_shares_issued)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        storage::write_total_insurance_tokens(&e, new_total_tokens);
        storage::write_total_insurance_shares(&e, new_total_shares);

        Ok(insurance_shares_issued)
    }

    /// Queue a withdrawal from the insurance pool.
    /// `amount_in_tokens` is the desired number of share tokens (lXLM etc) to withdraw.
    /// Earmarks the corresponding insurance shares but does NOT remove them from the pool —
    /// queued users continue to absorb bad debt events until execute_withdraw is called.
    /// The actual token amount received at execute time may differ if bad debt occurred.
    /// Returns the number of insurance shares earmarked.
    pub fn queue_withdraw(
        e: Env,
        user: Address,
        amount_in_tokens: i128,
    ) -> Result<i128, InsurancePoolError> {
        user.require_auth();

        if amount_in_tokens <= 0 {
            return Err(InsurancePoolError::NegativeAmount);
        }

        let total_tokens = storage::read_total_insurance_tokens(&e);
        let total_shares = storage::read_total_insurance_shares(&e);

        if total_shares == 0 {
            return Err(InsurancePoolError::ZeroTotalShares);
        }

        // Calculate insurance shares to earmark
        let shares_to_queue = amount_in_tokens
            .checked_mul(total_shares)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?
            .checked_div(total_tokens)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;

        // Validate user has enough insurance shares
        let positions = storage::read_insurance_positions(&e, &user);
        if positions.insurance_shares < shares_to_queue {
            return Err(InsurancePoolError::InsufficientInsuranceShares);
        }

        // Validate pool has enough tokens
        if total_tokens < amount_in_tokens {
            return Err(InsurancePoolError::InsufficientInsuranceTokens);
        }

        // Reject if queue already exists
        if storage::read_withdraw_queue(&e, &user).is_some() {
            return Err(InsurancePoolError::WithdrawQueueAlreadyExists);
        }

        // Record the queue entry — no accounting changes, shares remain in pool
        storage::write_withdraw_queue(
            &e,
            &user,
            WithdrawQueueEntry {
                queued_shares: shares_to_queue,
                queued_at_ledger: e.ledger().sequence(),
                queued_at_timestamp: e.ledger().timestamp(),
            },
        );

        Ok(shares_to_queue)
    }

    /// Cancel a pending withdrawal queue entry.
    /// No accounting changes needed since shares were never removed from the pool.
    pub fn cancel_queue(e: Env, user: Address) -> Result<(), InsurancePoolError> {
        user.require_auth();

        if storage::read_withdraw_queue(&e, &user).is_none() {
            return Err(InsurancePoolError::NoWithdrawQueue);
        }

        storage::delete_withdraw_queue(&e, &user);

        Ok(())
    }

    /// Execute a queued withdrawal after the 14-day waiting period has elapsed.
    /// Calculates the token value at the current pool rate (may be less than originally
    /// queued if bad debt occurred during the waiting period).
    /// Returns the number of share tokens transferred to the user.
    pub fn execute_withdraw(e: Env, user: Address) -> Result<i128, InsurancePoolError> {
        user.require_auth();

        let entry = storage::read_withdraw_queue(&e, &user)
            .ok_or(InsurancePoolError::NoWithdrawQueue)?;

        // Enforce 14-day waiting period
        if e.ledger().sequence() - entry.queued_at_ledger < storage::FOURTEEN_DAYS_IN_LEDGERS {
            return Err(InsurancePoolError::QueuePeriodNotElapsed);
        }

        let total_tokens = storage::read_total_insurance_tokens(&e);
        let total_shares = storage::read_total_insurance_shares(&e);

        // Validate user still has the earmarked shares (bad debt can't remove them, but
        // a future deposit could cause confusion — shares are per-user, so this is a safety check)
        let positions = storage::read_insurance_positions(&e, &user);
        if positions.insurance_shares < entry.queued_shares {
            return Err(InsurancePoolError::InsufficientInsuranceShares);
        }

        // Calculate token value at current pool rate (may be less if bad debt occurred)
        let tokens_out = entry
            .queued_shares
            .checked_mul(total_tokens)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?
            .checked_div(total_shares)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;

        if total_tokens < tokens_out {
            return Err(InsurancePoolError::InsufficientInsuranceTokens);
        }

        // Update user's insurance positions
        let new_shares = positions
            .insurance_shares
            .checked_sub(entry.queued_shares)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        storage::write_insurance_positions(&e, user.clone(), new_shares);

        // Update pool totals
        let new_total_tokens = total_tokens
            .checked_sub(tokens_out)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        let new_total_shares = total_shares
            .checked_sub(entry.queued_shares)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        storage::write_total_insurance_tokens(&e, new_total_tokens);
        storage::write_total_insurance_shares(&e, new_total_shares);

        // Transfer share tokens back to user
        let share_token_addr = storage::read_share_token_address(&e)?;
        let share_token_client = share_token::Client::new(&e, &share_token_addr);
        share_token_client.transfer(&e.current_contract_address(), &user, &tokens_out);

        storage::delete_withdraw_queue(&e, &user);

        Ok(tokens_out)
    }

    /// Returns the pending withdrawal queue entry for a user, if any.
    pub fn get_queue(e: Env, user: Address) -> Option<WithdrawQueueEntry> {
        storage::read_withdraw_queue(&e, &user)
    }

    /// Called by the paired loan pool to cover bad debt.
    /// Burns `ltoken_amount` share tokens held by this contract, reducing TotalInsuranceTokens.
    /// Insurance depositors absorb the loss via decreased ltoken-per-insurance-share ratio.
    pub fn cover_bad_debt(e: Env, ltoken_amount: i128) -> Result<(), InsurancePoolError> {
        let loan_pool_addr = storage::read_loan_pool_address(&e)?;
        loan_pool_addr.require_auth();

        if ltoken_amount <= 0 {
            return Err(InsurancePoolError::NegativeAmount);
        }

        let total_tokens = storage::read_total_insurance_tokens(&e);
        if total_tokens < ltoken_amount {
            return Err(InsurancePoolError::InsufficientInsuranceTokens);
        }

        // Burn the share tokens from this contract's holdings.
        // The insurance pool self-authorizes since it is the token holder.
        let share_token_addr = storage::read_share_token_address(&e)?;
        let share_token_client = share_token::Client::new(&e, &share_token_addr);
        share_token_client.burn(&e.current_contract_address(), &ltoken_amount);

        // Decrease total tokens — total shares unchanged, so each share is now worth less
        let new_total_tokens = total_tokens
            .checked_sub(ltoken_amount)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        storage::write_total_insurance_tokens(&e, new_total_tokens);

        Ok(())
    }

    /// Returns the current ltoken value of a user's insurance position.
    pub fn get_balance(e: Env, user: Address) -> Result<i128, InsurancePoolError> {
        let positions = storage::read_insurance_positions(&e, &user);
        let total_shares = storage::read_total_insurance_shares(&e);

        if total_shares == 0 {
            return Ok(0);
        }

        let total_tokens = storage::read_total_insurance_tokens(&e);
        let value = positions
            .insurance_shares
            .checked_mul(total_tokens)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?
            .checked_div(total_shares)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;

        Ok(value)
    }

    /// Returns the current state of the insurance pool.
    pub fn get_pool_state(e: Env) -> InsurancePoolState {
        InsurancePoolState {
            total_tokens: storage::read_total_insurance_tokens(&e),
            total_shares: storage::read_total_insurance_shares(&e),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger as _}, Env};

    mod share_token_wasm {
        soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/token.wasm");
    }

    fn setup(e: &Env) -> (InsurancePoolClient<'_>, Address, Address) {
        e.mock_all_auths();

        let insurance_pool_id = e.register(InsurancePool, ());
        let insurance_client = InsurancePoolClient::new(e, &insurance_pool_id);

        // Deploy a share token with the insurance pool as admin so it can burn
        let share_token_addr = e.register(
            share_token_wasm::WASM,
            (
                &insurance_pool_id,
                &7u32,
                &soroban_sdk::String::from_str(e, "lToken"),
                &soroban_sdk::String::from_str(e, "lT"),
            ),
        );

        let loan_pool_addr = Address::generate(e);

        insurance_client.initialize(&loan_pool_addr, &share_token_addr);

        (insurance_client, loan_pool_addr, share_token_addr)
    }

    /// Mint share tokens directly via the token contract (simulates loan pool deposit).
    fn mint_share_tokens(e: &Env, share_token_addr: &Address, insurance_pool_addr: &Address, to: &Address, amount: i128) {
        let token_client = share_token_wasm::Client::new(e, share_token_addr);
        // The insurance pool is the admin, so it can mint on behalf of any user.
        // We use mock_all_auths so the mint succeeds.
        token_client.mint(to, &amount);
        // Transfer from 'to' into the insurance pool to simulate the user having deposited
        // into the loan pool and now wanting to deposit those share tokens into insurance.
        // (Actual deposit call will do the transfer, so we just mint here.)
        let _ = insurance_pool_addr; // keep param for clarity
    }

    #[test]
    fn deposit_queue_and_execute_withdraw() {
        let e = Env::default();
        let (client, _loan_pool_addr, share_token_addr) = setup(&e);
        let insurance_pool_addr = client.address.clone();

        let user = Address::generate(&e);
        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &user, 1_000);

        // Deposit: 1:1 ratio → 1000 insurance shares
        let insurance_shares = client.deposit(&user, &1_000);
        assert_eq!(insurance_shares, 1_000);

        let balance = client.get_balance(&user);
        assert_eq!(balance, 1_000);

        // Queue a withdrawal of 500 tokens worth
        let queued_shares = client.queue_withdraw(&user, &500);
        assert_eq!(queued_shares, 500);

        // Pool totals unchanged — shares still in pool
        let state = client.get_pool_state();
        assert_eq!(state.total_tokens, 1_000);
        assert_eq!(state.total_shares, 1_000);

        // Balance unchanged — queued shares still in pool
        let balance_after_queue = client.get_balance(&user);
        assert_eq!(balance_after_queue, 1_000);

        // get_queue returns the entry
        let queue_entry = client.get_queue(&user).expect("queue entry should exist");
        assert_eq!(queue_entry.queued_shares, 500);

        // Advance ledger by 14 days
        e.ledger().with_mut(|l| {
            l.sequence_number += storage::FOURTEEN_DAYS_IN_LEDGERS;
        });

        // Execute withdraw
        let tokens_out = client.execute_withdraw(&user);
        assert_eq!(tokens_out, 500);

        let state2 = client.get_pool_state();
        assert_eq!(state2.total_tokens, 500);
        assert_eq!(state2.total_shares, 500);

        // Queue entry should be gone
        assert!(client.get_queue(&user).is_none());
    }

    #[test]
    fn cancel_queue_restores_no_change() {
        let e = Env::default();
        let (client, _loan_pool_addr, share_token_addr) = setup(&e);
        let insurance_pool_addr = client.address.clone();

        let user = Address::generate(&e);
        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &user, 1_000);
        client.deposit(&user, &1_000);

        // Queue then cancel
        client.queue_withdraw(&user, &500);
        assert!(client.get_queue(&user).is_some());

        client.cancel_queue(&user);
        assert!(client.get_queue(&user).is_none());

        // Pool state unchanged
        let state = client.get_pool_state();
        assert_eq!(state.total_tokens, 1_000);
        assert_eq!(state.total_shares, 1_000);

        let balance = client.get_balance(&user);
        assert_eq!(balance, 1_000);
    }

    #[test]
    fn queue_collision_errors() {
        let e = Env::default();
        let (client, _loan_pool_addr, share_token_addr) = setup(&e);
        let insurance_pool_addr = client.address.clone();

        let user = Address::generate(&e);
        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &user, 1_000);
        client.deposit(&user, &1_000);

        client.queue_withdraw(&user, &300);

        // Second queue_withdraw should fail
        let result = client.try_queue_withdraw(&user, &200);
        assert!(result.is_err());
    }

    #[test]
    fn execute_before_14_days_errors() {
        let e = Env::default();
        let (client, _loan_pool_addr, share_token_addr) = setup(&e);
        let insurance_pool_addr = client.address.clone();

        let user = Address::generate(&e);
        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &user, 1_000);
        client.deposit(&user, &1_000);
        client.queue_withdraw(&user, &500);

        // Only 7 days elapsed
        e.ledger().with_mut(|l| {
            l.sequence_number += storage::FOURTEEN_DAYS_IN_LEDGERS / 2;
        });

        let result = client.try_execute_withdraw(&user);
        assert!(result.is_err());
    }

    #[test]
    fn bad_debt_absorbed_during_queue() {
        let e = Env::default();
        let (client, _loan_pool_addr, share_token_addr) = setup(&e);
        let insurance_pool_addr = client.address.clone();

        let user = Address::generate(&e);
        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &user, 1_000);
        client.deposit(&user, &1_000);

        // Queue a withdrawal of all 1000 tokens (= 1000 shares)
        let queued_shares = client.queue_withdraw(&user, &1_000);
        assert_eq!(queued_shares, 1_000);

        // Bad debt of 200 tokens occurs during the queue period
        client.cover_bad_debt(&200);

        let state = client.get_pool_state();
        assert_eq!(state.total_tokens, 800); // reduced by bad debt
        assert_eq!(state.total_shares, 1_000); // unchanged

        // Advance past 14 days
        e.ledger().with_mut(|l| {
            l.sequence_number += storage::FOURTEEN_DAYS_IN_LEDGERS;
        });

        // Execute: receives only 800 (absorbed the 200 bad debt loss)
        let tokens_out = client.execute_withdraw(&user);
        assert_eq!(tokens_out, 800);

        let state2 = client.get_pool_state();
        assert_eq!(state2.total_tokens, 0);
        assert_eq!(state2.total_shares, 0);
    }

    #[test]
    fn cover_bad_debt_preserves_regular_depositor_ratio() {
        let e = Env::default();
        let (client, loan_pool_addr, share_token_addr) = setup(&e);
        let insurance_pool_addr = client.address.clone();

        // Two insurance depositors each put in 500 share tokens (total 1000)
        let alice = Address::generate(&e);
        let bob = Address::generate(&e);
        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &alice, 500);
        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &bob, 500);

        client.deposit(&alice, &500);
        client.deposit(&bob, &500);

        let state = client.get_pool_state();
        assert_eq!(state.total_tokens, 1_000);
        assert_eq!(state.total_shares, 1_000);

        // Simulate loan pool calling cover_bad_debt for 200 tokens of bad debt.
        let _ = loan_pool_addr; // used in setup; mock_all_auths covers it
        client.cover_bad_debt(&200);

        // TotalInsuranceTokens decreases; TotalInsuranceShares stays the same
        let state_after = client.get_pool_state();
        assert_eq!(state_after.total_tokens, 800);
        assert_eq!(state_after.total_shares, 1_000); // shares unchanged

        // Alice's balance is now 400 (500/1000 * 800)
        let alice_balance = client.get_balance(&alice);
        assert_eq!(alice_balance, 400);

        // Bob's balance is 400 as well
        let bob_balance = client.get_balance(&bob);
        assert_eq!(bob_balance, 400);
    }

    #[test]
    fn second_depositor_proportional_shares() {
        let e = Env::default();
        let (client, _loan_pool_addr, share_token_addr) = setup(&e);
        let insurance_pool_addr = client.address.clone();

        let alice = Address::generate(&e);
        let bob = Address::generate(&e);

        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &alice, 1_000);
        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &bob, 500);

        // Alice deposits first: 1000 tokens → 1000 insurance shares (1:1)
        let alice_shares = client.deposit(&alice, &1_000);
        assert_eq!(alice_shares, 1_000);

        // Bob deposits 500 tokens when pool has 1000 tokens / 1000 shares → 1:1 still
        let bob_shares = client.deposit(&bob, &500);
        assert_eq!(bob_shares, 500);

        let state = client.get_pool_state();
        assert_eq!(state.total_tokens, 1_500);
        assert_eq!(state.total_shares, 1_500);

        // Alice queues and executes a full withdrawal after 14 days
        client.queue_withdraw(&alice, &1_000);
        e.ledger().with_mut(|l| {
            l.sequence_number += storage::FOURTEEN_DAYS_IN_LEDGERS;
        });
        let alice_tokens_out = client.execute_withdraw(&alice);
        assert_eq!(alice_tokens_out, 1_000);

        let state2 = client.get_pool_state();
        assert_eq!(state2.total_tokens, 500);
        assert_eq!(state2.total_shares, 500);
    }
}
