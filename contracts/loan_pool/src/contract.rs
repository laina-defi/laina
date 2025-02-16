use crate::dto::PoolState;
use crate::error::LoanPoolError;
use crate::interest::{self, get_interest};
use crate::pool::Currency;
use crate::positions;
use crate::storage_types::PoolDataKey;
use crate::{pool, storage_types::Positions};

use soroban_sdk::{
    contract, contractimpl, contractmeta, symbol_short, token, Address, BytesN, Env,
};

// Metadata that is added on to the WASM custom section
contractmeta!(
    key = "Desc",
    val = "Lending pool with variable interest rate."
);

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
    ) {
        pool::write_loan_manager_addr(&e, loan_manager_addr);
        pool::write_currency(&e, currency);
        pool::write_liquidation_threshold(&e, liquidation_threshold);
        pool::write_total_shares(&e, 0);
        pool::write_total_balance(&e, 0);
        pool::write_available_balance(&e, 0);
        pool::write_accrual(&e, 10_000_000); // Default initial accrual value.
        pool::write_accrual_last_updated(&e, e.ledger().timestamp());
        pool::change_interest_rate_multiplier(&e, 1); // Temporary parameter
    }

    pub fn upgrade(e: Env, new_wasm_hash: BytesN<32>) -> Result<(), LoanPoolError> {
        let loan_manager_addr = pool::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        e.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    pub fn change_interest_rate_multiplier(e: Env, multiplier: i128) -> Result<(), LoanPoolError> {
        let loan_manager_addr = pool::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        pool::change_interest_rate_multiplier(&e, multiplier);
        Ok(())
    }

    /// Deposits token. Also, mints pool shares for the "user" Identifier.
    pub fn deposit(e: Env, user: Address, amount: i128) -> Result<i128, LoanPoolError> {
        user.require_auth();
        if amount <= 0 {
            Err(LoanPoolError::NegativeDeposit)
        } else {
            Self::add_interest_to_accrual(e.clone())?;

            let token_address = pool::read_currency(&e)?.token_address;

            let client = token::Client::new(&e, &token_address);
            client.transfer(&user, &e.current_contract_address(), &amount);

            pool::change_available_balance(&e, amount)?;
            pool::change_total_shares(&e, amount)?;
            pool::change_total_balance(&e, amount)?;

            // Increase users position in pool as they deposit
            // as this is deposit amount is added to receivables and
            // liabilities & collateral stays intact
            let liabilities: i128 = 0; // temp test param
            let collateral: i128 = 0; // temp test param
            positions::increase_positions(&e, user.clone(), amount, liabilities, collateral)?;

            Ok(amount)
        }
    }

    /// Transfers share tokens back, burns them and gives corresponding amount of tokens back to user. Returns amount of tokens withdrawn
    pub fn withdraw(e: Env, user: Address, amount: i128) -> Result<PoolState, LoanPoolError> {
        user.require_auth();

        Self::add_interest_to_accrual(e.clone())?;

        // Get users receivables
        let Positions {
            receivable_shares, ..
        } = positions::read_positions(&e, &user);

        // Check that user is not trying to move more than receivables (TODO: also include collateral?)
        if amount > receivable_shares {
            return Err(LoanPoolError::WithdrawIsNegative);
        }

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

        let new_available_balance_tokens = pool::change_available_balance(
            &e,
            amount.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        let new_total_balance_tokens = pool::change_total_balance(
            &e,
            amount.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;
        let new_total_balance_shares = pool::change_total_shares(&e, shares_to_decrease)?;
        let liabilities: i128 = 0;
        let collateral: i128 = 0;
        positions::decrease_positions(
            &e,
            user.clone(),
            shares_to_decrease,
            liabilities,
            collateral,
        )?;

        // Transfer tokens from pool to user
        let token_address = &pool::read_currency(&e)?.token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&e.current_contract_address(), &user, &amount);

        let new_annual_interest_rate = Self::get_interest(e.clone())?;

        let pool_state = PoolState {
            total_balance_tokens: new_total_balance_tokens,
            available_balance_tokens: new_available_balance_tokens,
            total_balance_shares: new_total_balance_shares,
            annual_interest_rate: new_annual_interest_rate,
        };
        Ok(pool_state)
    }

    /// Borrow tokens from the pool
    pub fn borrow(e: Env, user: Address, amount: i128) -> Result<i128, LoanPoolError> {
        /*
        Borrow should only be callable from the loans contract. This is as the loans contract will
        include the logic and checks that the borrowing can be actually done. Therefore we need to
        include a check that the caller is the loans contract.
        */
        let loan_manager_addr = pool::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();
        user.require_auth();

        Self::add_interest_to_accrual(e.clone())?;

        let balance = pool::read_available_balance(&e)?;
        assert!(
            amount < balance,
            "Borrowed amount has to be less than available balance!"
        ); // Check that there is enough available balance

        pool::change_available_balance(
            &e,
            amount.checked_neg().ok_or(LoanPoolError::OverOrUnderFlow)?,
        )?;

        // Increase users position in pool as they deposit
        // as this is debt amount is added to liabilities and
        // collateral & receivables stays intact
        let collateral: i128 = 0; // temp test param
        let receivables: i128 = 0; // temp test param
        positions::increase_positions(&e, user.clone(), receivables, amount, collateral)?;

        let token_address = &pool::read_currency(&e)?.token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&e.current_contract_address(), &user, &amount);

        Ok(amount)
    }

    /// Deposit tokens to the pool to be used as collateral
    pub fn deposit_collateral(e: Env, user: Address, amount: i128) -> Result<i128, LoanPoolError> {
        user.require_auth();
        assert!(amount > 0, "Amount must be positive!");

        Self::add_interest_to_accrual(e.clone())?;

        let token_address = &pool::read_currency(&e)?.token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&user, &e.current_contract_address(), &amount);

        // Increase users position in pool as they deposit
        // as this is collateral amount is added to collateral and
        // liabilities & receivables stays intact
        let liabilities: i128 = 0; // temp test param
        let receivables: i128 = 0; // temp test param
        positions::increase_positions(&e, user.clone(), receivables, liabilities, amount)?;

        Ok(amount)
    }

    pub fn withdraw_collateral(e: Env, user: Address, amount: i128) -> Result<i128, LoanPoolError> {
        user.require_auth();
        Self::add_interest_to_accrual(e.clone())?;

        let loan_manager_addr = pool::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();
        assert!(amount > 0, "Amount must be positive!");

        let token_address = &pool::read_currency(&e)?.token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&e.current_contract_address(), &user, &amount);

        // Increase users position in pool as they deposit
        // as this is collateral amount is added to collateral and
        // liabilities & receivables stays intact
        let liabilities: i128 = 0; // temp test param
        let receivables: i128 = 0; // temp test param
        positions::decrease_positions(&e, user.clone(), receivables, liabilities, amount)?;

        Ok(amount)
    }

    pub fn add_interest_to_accrual(e: Env) -> Result<(), LoanPoolError> {
        const DECIMAL: i128 = 10000000;
        const SECONDS_IN_YEAR: u64 = 31_556_926;

        let current_timestamp = e.ledger().timestamp();
        let accrual = pool::read_accrual(&e)?;
        let accrual_last_update = pool::read_accrual_last_updated(&e)?;
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

        pool::write_accrual_last_updated(&e, current_timestamp);
        e.events().publish(
            (PoolDataKey::AccrualLastUpdate, symbol_short!("updated")),
            current_timestamp,
        );

        pool::write_accrual(&e, new_accrual);
        e.events().publish(
            (PoolDataKey::Accrual, symbol_short!("updated")),
            new_accrual,
        );
        Ok(())
    }

    pub fn get_accrual(e: &Env) -> Result<i128, LoanPoolError> {
        pool::read_accrual(e)
    }

    pub fn get_collateral_factor(e: &Env) -> Result<i128, LoanPoolError> {
        pool::read_collateral_factor(e)
    }

    /// Get user's positions in the pool
    pub fn get_user_positions(e: Env, user: Address) -> Positions {
        positions::read_positions(&e, &user)
    }

    /// Get contract data entries
    pub fn get_contract_balance(e: Env) -> Result<i128, LoanPoolError> {
        pool::read_total_balance(&e)
    }

    pub fn get_total_balance_shares(e: Env) -> Result<i128, LoanPoolError> {
        pool::read_total_shares(&e)
    }

    pub fn get_available_balance(e: Env) -> Result<i128, LoanPoolError> {
        pool::read_available_balance(&e)
    }

    pub fn get_currency(e: Env) -> Result<Currency, LoanPoolError> {
        pool::read_currency(&e)
    }

    pub fn get_interest(e: Env) -> Result<i128, LoanPoolError> {
        interest::get_interest(e)
    }

    pub fn get_pool_state(e: Env) -> Result<PoolState, LoanPoolError> {
        Ok(PoolState {
            total_balance_tokens: pool::read_total_balance(&e)?,
            available_balance_tokens: pool::read_available_balance(&e)?,
            total_balance_shares: pool::read_total_shares(&e)?,
            annual_interest_rate: interest::get_interest(e)?,
        })
    }

    pub fn increase_liabilities(e: Env, user: Address, amount: i128) -> Result<(), LoanPoolError> {
        let loan_manager_addr = pool::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        positions::increase_positions(&e, user.clone(), 0, amount, 0)?;
        e.events().publish(
            (
                PoolDataKey::Positions(user),
                symbol_short!("liab"),
                symbol_short!("incr"),
            ),
            amount,
        );
        Ok(())
    }

    pub fn repay(
        e: Env,
        user: Address,
        amount: i128,
        unpaid_interest: i128,
    ) -> Result<(), LoanPoolError> {
        let loan_manager_addr = pool::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        Self::add_interest_to_accrual(e.clone())?;

        let amount_to_admin = if amount < unpaid_interest {
            amount / 10
        } else {
            unpaid_interest / 10
        };

        let amount_to_pool = amount
            .checked_sub(amount_to_admin)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let client = token::Client::new(&e, &pool::read_currency(&e)?.token_address);
        client.transfer(&user, &e.current_contract_address(), &amount_to_pool);
        client.transfer(&user, &loan_manager_addr, &amount_to_admin);

        positions::decrease_positions(&e, user, 0, amount, 0)?;
        pool::change_available_balance(&e, amount - amount_to_admin)?;
        pool::change_total_balance(&e, unpaid_interest - amount_to_admin)?;
        Ok(())
    }

    pub fn repay_and_close(
        e: Env,
        user: Address,
        borrowed_amount: i128,
        max_allowed_amount: i128,
        unpaid_interest: i128,
    ) -> Result<(), LoanPoolError> {
        let loan_manager_addr = pool::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        Self::add_interest_to_accrual(e.clone())?;

        let amount_to_admin = if borrowed_amount < unpaid_interest {
            borrowed_amount / 10
        } else {
            unpaid_interest / 10
        };

        let amount_to_user = max_allowed_amount
            .checked_sub(borrowed_amount)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let client = token::Client::new(&e, &pool::read_currency(&e)?.token_address);
        client.transfer(&user, &e.current_contract_address(), &max_allowed_amount);
        client.transfer(
            &e.current_contract_address(),
            &loan_manager_addr,
            &amount_to_admin,
        );
        client.transfer(&e.current_contract_address(), &user, &amount_to_user);

        let user_liabilities = positions::read_positions(&e, &user).liabilities;
        positions::decrease_positions(&e, user, 0, user_liabilities, 0)?;
        pool::change_available_balance(&e, borrowed_amount - amount_to_admin)?;
        pool::change_total_balance(&e, unpaid_interest - amount_to_admin)?;
        Ok(())
    }

    pub fn liquidate(
        e: Env,
        user: Address,
        amount: i128,
        unpaid_interest: i128,
        loan_owner: Address,
    ) -> Result<(), LoanPoolError> {
        let loan_manager_addr = pool::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        Self::add_interest_to_accrual(e.clone())?;

        let amount_to_admin = if amount < unpaid_interest {
            amount / 10
        } else {
            unpaid_interest / 10
        };

        let amount_to_pool = amount
            .checked_sub(amount_to_admin)
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let client = token::Client::new(&e, &pool::read_currency(&e)?.token_address);
        client.transfer(&user, &e.current_contract_address(), &amount_to_pool);
        client.transfer(&user, &loan_manager_addr, &amount_to_admin);

        positions::decrease_positions(&e, loan_owner, 0, amount, 0)?;
        pool::change_available_balance(&e, amount)?;
        pool::change_total_balance(&e, amount)?;
        Ok(())
    }

    pub fn liquidate_transfer_collateral(
        e: Env,
        user: Address,
        amount_collateral: i128,
        loan_owner: Address,
    ) -> Result<(), LoanPoolError> {
        let loan_manager_addr = pool::read_loan_manager_addr(&e)?;
        loan_manager_addr.require_auth();

        let client = token::Client::new(&e, &pool::read_currency(&e)?.token_address);
        client.transfer(&e.current_contract_address(), &user, &amount_collateral);

        positions::decrease_positions(&e, loan_owner, 0, 0, amount_collateral)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*; // This imports LoanPoolContract and everything else from the parent module
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::{Client as TokenClient, StellarAssetClient},
        Env, Symbol,
    };

    const TEST_LIQUIDATION_THRESHOLD: i128 = 8_000_000;

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

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
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
        let amount: i128 = 100;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
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
        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
        );

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
        let amount: i128 = 100;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
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

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
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
        let amount: i128 = 2000;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
        );

        contract_client.deposit(&user, &amount);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #12)")]
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
        let amount: i128 = 100;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
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
        let amount: i128 = 100;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
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
        let amount: i128 = 1000;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
        );

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
        let amount: i128 = 1000;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
        );

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
