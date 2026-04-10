use soroban_sdk::{contract, contractimpl, Address, Env};

use crate::{error::InsurancePoolError, storage};

mod share_token {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/token.wasm");
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

    /// Withdraw share tokens from the insurance pool.
    /// `amount_in_tokens` is the number of share tokens (lXLM etc) to withdraw.
    /// Returns the number of insurance shares burned.
    pub fn withdraw(
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

        // Calculate insurance shares to burn
        let shares_to_burn = amount_in_tokens
            .checked_mul(total_shares)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?
            .checked_div(total_tokens)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;

        // Validate user has enough insurance shares
        let positions = storage::read_insurance_positions(&e, &user);
        if positions.insurance_shares < shares_to_burn {
            return Err(InsurancePoolError::InsufficientInsuranceShares);
        }

        // Validate pool has enough tokens
        if total_tokens < amount_in_tokens {
            return Err(InsurancePoolError::InsufficientInsuranceTokens);
        }

        // Update user's insurance positions
        let new_shares = positions
            .insurance_shares
            .checked_sub(shares_to_burn)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        storage::write_insurance_positions(&e, user.clone(), new_shares);

        // Update totals
        let new_total_tokens = total_tokens
            .checked_sub(amount_in_tokens)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        let new_total_shares = total_shares
            .checked_sub(shares_to_burn)
            .ok_or(InsurancePoolError::OverOrUnderFlow)?;
        storage::write_total_insurance_tokens(&e, new_total_tokens);
        storage::write_total_insurance_shares(&e, new_total_shares);

        // Transfer share tokens back to user
        let share_token_addr = storage::read_share_token_address(&e)?;
        let share_token_client = share_token::Client::new(&e, &share_token_addr);
        share_token_client.transfer(&e.current_contract_address(), &user, &amount_in_tokens);

        Ok(shares_to_burn)
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

    /// Returns (total_tokens, total_shares) for the insurance pool.
    pub fn get_pool_state(e: Env) -> (i128, i128) {
        (
            storage::read_total_insurance_tokens(&e),
            storage::read_total_insurance_shares(&e),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

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
    fn deposit_and_withdraw() {
        let e = Env::default();
        let (client, _loan_pool_addr, share_token_addr) = setup(&e);
        let insurance_pool_addr = client.address.clone();

        let user = Address::generate(&e);
        // Mint 1000 share tokens to the user (simulating prior loan pool deposit)
        mint_share_tokens(&e, &share_token_addr, &insurance_pool_addr, &user, 1_000);

        // First deposit: 1:1 ratio → should issue 1000 insurance shares
        let insurance_shares = client.deposit(&user, &1_000);
        assert_eq!(insurance_shares, 1_000);

        let (total_tokens, total_shares) = client.get_pool_state();
        assert_eq!(total_tokens, 1_000);
        assert_eq!(total_shares, 1_000);

        let balance = client.get_balance(&user);
        assert_eq!(balance, 1_000);

        // Withdraw 500 tokens worth
        let shares_burned = client.withdraw(&user, &500);
        assert_eq!(shares_burned, 500);

        let (total_tokens2, total_shares2) = client.get_pool_state();
        assert_eq!(total_tokens2, 500);
        assert_eq!(total_shares2, 500);
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

        let (total_tokens, total_shares) = client.get_pool_state();
        assert_eq!(total_tokens, 1_000);
        assert_eq!(total_shares, 1_000);

        // Simulate loan pool calling cover_bad_debt for 200 tokens of bad debt.
        // Auth is from loan_pool_addr (mocked).
        let _ = loan_pool_addr; // used in setup; mock_all_auths covers it
        client.cover_bad_debt(&200);

        // TotalInsuranceTokens decreases; TotalInsuranceShares stays the same
        let (total_tokens_after, total_shares_after) = client.get_pool_state();
        assert_eq!(total_tokens_after, 800);
        assert_eq!(total_shares_after, 1_000); // shares unchanged

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

        let (total_tokens, total_shares) = client.get_pool_state();
        assert_eq!(total_tokens, 1_500);
        assert_eq!(total_shares, 1_500);

        // Alice withdraws all her tokens (1000 worth)
        let alice_burned = client.withdraw(&alice, &1_000);
        assert_eq!(alice_burned, 1_000);

        let (total_tokens2, total_shares2) = client.get_pool_state();
        assert_eq!(total_tokens2, 500);
        assert_eq!(total_shares2, 500);
    }
}
