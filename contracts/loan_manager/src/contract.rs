use crate::error::LoanManagerError;
use crate::oracle::{self, Asset};
use crate::storage::{self, Loan, LoanId, NewLoan};

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
        );

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

        let collateral_shares_amount = collateral_pool_client.get_shares_from_tokens(&collateral);

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

        // Health factor has to be over 1.2 for the loan to be initialized.
        // Health factor is defined as so: 1.0 = 10000000_i128
        const HEALTH_FACTOR_THRESHOLD: i128 = 10000000;
        assert!(
            health_factor > HEALTH_FACTOR_THRESHOLD,
            "Health factor must be over {HEALTH_FACTOR_THRESHOLD} to create a new loan!"
        );

        // Deposit collateral
        collateral_pool_client.deposit_collateral(&user, &collateral);

        // Borrow the funds
        let borrowed_amount = borrow_pool_client.borrow(&user, &borrowed);

        let unpaid_interest = 0;

        let new_loan = NewLoan {
            borrower_address: user.clone(),
            borrowed_amount,
            borrowed_from,
            collateral_amount: collateral_shares_amount,
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
            collateral_amount,
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

        let collateral_amount_tokens =
            collateral_pool_client.get_tokens_from_shares(&collateral_amount);
        let new_health_factor = Self::calculate_health_factor(
            e,
            token_ticker,
            new_borrowed_amount,
            token_collateral_ticker,
            collateral_amount_tokens,
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
            collateral_amount,
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

        let amount_of_data_points = 12; // 12 * 5 min = 1h average
        let collateral_asset_price = reflector_contract
            .twap(&collateral_asset, &amount_of_data_points)
            .ok_or(LoanManagerError::NoLastPrice)?;
        let collateral_value = collateral_asset_price
            .checked_mul(token_collateral_amount)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_mul(collateral_factor)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(DECIMAL_TO_INT_MULTIPLIER)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        // get the price and calculate the value of the borrowed asset
        let borrowed_asset = Asset::Other(token_ticker);
        let asset_price = reflector_contract
            .twap(&borrowed_asset, &amount_of_data_points)
            .ok_or(LoanManagerError::NoLastPrice)?;
        let borrowed_value = asset_price
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
            collateral_amount,
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

        let collateral_amount_tokens =
            collateral_pool_client.get_tokens_from_shares(&collateral_amount);
        let new_health_factor = Self::calculate_health_factor(
            e,
            borrow_pool_client.get_currency().ticker,
            new_borrowed_amount,
            collateral_pool_client.get_currency().ticker,
            collateral_amount_tokens,
            collateral_from.clone(),
        )?;

        storage::write_loan(
            e,
            &loan_id,
            &Loan {
                loan_id: loan_id.clone(),
                borrowed_amount: new_borrowed_amount,
                borrowed_from,
                collateral_amount,
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
            collateral_amount,
            collateral_from,
            unpaid_interest,
            ..
        } = Self::add_interest(e, loan_id.clone())?;

        let borrow_pool_client = loan_pool::Client::new(e, &borrowed_from);
        borrow_pool_client.repay_and_close(
            &user,
            &borrowed_amount,
            &max_allowed_amount,
            &unpaid_interest,
        );

        let collateral_pool_client = loan_pool::Client::new(e, &collateral_from);
        let collateral_amount_tokens =
            collateral_pool_client.get_tokens_from_shares(&collateral_amount);
        collateral_pool_client.withdraw_collateral(
            &user,
            &collateral_amount_tokens,
            &collateral_amount,
        );

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
            collateral_amount,
            unpaid_interest,
            last_accrual,
            ..
        } = Self::add_interest(&e, loan_id.clone())?;

        let borrow_pool_client = loan_pool::Client::new(&e, &borrowed_from);
        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_from);

        let borrowed_ticker = borrow_pool_client.get_currency().ticker;
        let collateral_ticker = collateral_pool_client.get_currency().ticker;

        // Check that loan is for sure liquidatable at this moment.
        let collateral_amount_tokens =
            collateral_pool_client.get_tokens_from_shares(&collateral_amount);
        let health_factor_before_liquidation = Self::calculate_health_factor(
            &e,
            borrowed_ticker.clone(),
            borrowed_amount,
            collateral_ticker.clone(),
            collateral_amount_tokens,
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
        let collateral_amount_bonus_shares =
            collateral_pool_client.get_shares_from_tokens(&collateral_amount_bonus);

        borrow_pool_client.liquidate(&user, &amount, &unpaid_interest, &loan_id.borrower_address);

        collateral_pool_client.liquidate_transfer_collateral(
            &user,
            &collateral_amount_bonus,
            &collateral_amount_bonus_shares,
            &loan_id.borrower_address,
        );

        let new_borrowed_amount = borrowed_amount
            .checked_sub(amount)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;
        let new_collateral_amount = collateral_amount
            .checked_sub(collateral_amount_bonus_shares)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let new_collateral_amount_tokens =
            collateral_pool_client.get_tokens_from_shares(&new_collateral_amount);
        let new_health_factor = Self::calculate_health_factor(
            &e,
            borrowed_ticker,
            new_borrowed_amount,
            collateral_ticker,
            new_collateral_amount_tokens,
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
            collateral_amount: new_collateral_amount,
            health_factor: new_health_factor,
            unpaid_interest, // Temp
            last_accrual,
        };

        storage::write_loan(&e, &loan_id, &new_loan);

        Ok(new_loan)
    }
}

#[cfg(test)]
mod tests {
    use crate::contract::loan_pool::{PoolState, Positions};

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
        assert_eq!(usdc_balance, 100_000);
        let eurc_balance = pool_eurc_client.get_contract_balance();
        assert_eq!(eurc_balance, 100_000);
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
        pool_client.initialize(
            &manager_addr,
            &Currency {
                ticker,
                token_address,
            },
            &8_000_000,
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
        pool_client.initialize(
            &manager_addr,
            &Currency {
                ticker,
                token_address,
            },
            &8_000_000,
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

        xlm_asset_client.mint(&admin, &900_001);
        pool_xlm_client.deposit(&admin, &900_001);
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
        assert_eq!(user_loan.collateral_amount, 100_000);

        manager_client.repay(&loan.loan_id, &100);
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000 + 1;
        });

        let user_loan = manager_client.get_loan(&loan.loan_id);

        assert_eq!(user_loan.borrowed_amount, 920);
        assert_eq!(user_loan.collateral_amount, 100_000);
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
        assert_eq!(pool_usdc_client.get_available_balance(), 99_900);

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
        assert_eq!(loan.collateral_amount, 500);

        manager_client.repay(&loan.loan_id, &50);
        loan = manager_client.get_loan(&loan.loan_id);
        assert_eq!(loan.borrowed_amount, 52);

        assert_eq!((52, 2), manager_client.repay(&loan.loan_id, &50));
        assert_eq!(100_000, pool_usdc_client.get_available_balance());
        assert_eq!(100_002, pool_usdc_client.get_contract_balance());
        assert_eq!(100_000, pool_usdc_client.get_total_balance_shares());

        // Create a new user that should not be able to withdraw more than what they have deposited even if the pool already has interest
        let new_user = Address::generate(&e);
        usdc_asset_client.mint(&new_user, &1_000);

        pool_usdc_client.deposit(&new_user, &1000);
        assert_eq!(101002, pool_usdc_client.get_contract_balance());
        let positions_new_user = Positions {
            collateral: 0,
            liabilities: 0,
            receivable_shares: 999,
        };
        assert_eq!(
            positions_new_user,
            pool_usdc_client.get_user_positions(&new_user)
        );

        let test_positions_admin = Positions {
            collateral: 0,
            liabilities: 0,
            receivable_shares: 100_000,
        };
        assert_eq!(
            test_positions_admin,
            pool_usdc_client.get_user_positions(&admin)
        );
        let pool_state = PoolState {
            annual_interest_rate: 200_017,
            available_balance_tokens: 101_000,
            total_balance_shares: 100_999,
            total_balance_tokens: 101_002,
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
        assert_eq!(loan_usdc.collateral_amount, 100);
        assert_eq!(loan_usdc.borrowed_from, pool_usdc_addr);
        assert_eq!(loan_usdc.collateral_from, pool_xlm_addr);

        let loan_eurc = loans.get(1).unwrap();
        assert_eq!(loan_eurc.borrowed_amount, 30);
        assert_eq!(loan_eurc.collateral_amount, 300);
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
        assert_eq!(loan.collateral_amount, 1000);
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
        assert_eq!(loan_usdc.collateral_amount, 500);

        manager_client.repay(&loan_usdc.loan_id, &50);
        loan_usdc = manager_client.get_loan(&loan_usdc.loan_id);
        assert_eq!(loan_usdc.borrowed_amount, 52);
        assert_eq!(loan_usdc.collateral_amount, 500);

        assert_eq!((52, 2), manager_client.repay(&loan_usdc.loan_id, &50));
        assert_eq!(100_000, pool_usdc_client.get_available_balance());
        assert_eq!(100_002, pool_usdc_client.get_contract_balance());
        assert_eq!(100_000, pool_usdc_client.get_total_balance_shares());

        loan_eurc = manager_client.get_loan(&loan_eurc.loan_id);
        assert_eq!(loan_eurc.borrowed_amount, 100);
        assert_eq!(loan_eurc.collateral_amount, 500);
        assert_eq!(99_900, pool_eurc_client.get_available_balance());
        assert_eq!(100_000, pool_eurc_client.get_contract_balance());
        assert_eq!(100_000, pool_eurc_client.get_total_balance_shares());
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
        assert_eq!(100002, pool_usdc_client.get_available_balance());
        assert_eq!(100002, pool_usdc_client.get_contract_balance());
        assert_eq!(100000, pool_usdc_client.get_total_balance_shares());
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
            pool_xlm_client,
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
        assert_eq!(usdc_loan.collateral_amount, 300);

        // mint the user some money so they can repay.
        usdc_asset_client.mint(&user, &45);
        assert_eq!(
            102,
            manager_client
                .repay_and_close_manager(&(usdc_loan.borrowed_amount + 45), &usdc_loan.loan_id)
        );

        assert_eq!(100002, pool_usdc_client.get_available_balance());
        assert_eq!(100002, pool_usdc_client.get_contract_balance());
        assert_eq!(100000, pool_usdc_client.get_total_balance_shares());
        assert_eq!(43, usdc_token_client.balance(&user));

        assert_eq!(300, pool_xlm_client.get_available_balance());

        // The eurc loan should still be there after repaying the usdc loan
        let loans = manager_client.get_loans(&user);
        assert_eq!(loans.len(), 1);
        let eurc_loan = loans.get(0).unwrap();
        assert_eq!(eurc_loan.borrowed_amount, 100);
        assert_eq!(eurc_loan.collateral_amount, 300);
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
        usdc_asset_client.mint(&admin, &900_001);
        eurc_asset_client.mint(&admin, &900_001);
        xlm_asset_client.mint(&user, &300_000);
        pool_usdc_client.deposit(&admin, &900_001);
        pool_eurc_client.deposit(&admin, &900_001);

        // ACT
        // Create two loans, one to liquidate.
        let mut usdc_loan =
            manager_client.create_loan(&user, &100_000, &pool_usdc_addr, &125_050, &pool_xlm_addr);
        let mut eurc_loan =
            manager_client.create_loan(&user, &100_000, &pool_eurc_addr, &125_050, &pool_xlm_addr);

        manager_client.add_interest(&usdc_loan.loan_id);
        manager_client.add_interest(&eurc_loan.loan_id);

        // Here borrowed amount should be the same as time has not moved. add_interest() is only called to store the LastUpdate sequence number.
        assert_eq!(usdc_loan.borrowed_amount, 100_000);
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

        assert_eq!(usdc_loan.borrowed_amount, 100_732);
        assert_eq!(usdc_loan.health_factor, 9_931_302);
        assert_eq!(usdc_loan.collateral_amount, 125_050);

        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 1_000;
        });

        e.register_at(&reflector_addr, oracle::WASM, ());

        manager_client.liquidate(&admin, &usdc_loan.loan_id, &5_000);

        usdc_loan = manager_client.get_loan(&usdc_loan.loan_id);
        assert_eq!(usdc_loan.borrowed_amount, 95_732);
        assert_eq!(usdc_loan.health_factor, 9_990_389);
        assert_eq!(usdc_loan.collateral_amount, 119_550);

        eurc_loan = manager_client.get_loan(&eurc_loan.loan_id);
        assert_eq!(eurc_loan.borrowed_amount, 100_732);
        assert_eq!(eurc_loan.health_factor, 9_931_302);
        assert_eq!(eurc_loan.collateral_amount, 125_050);
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
    fn create_loan_with_shares() {
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
            admin,
            user,
            user2,
            manager_client,
            pool_xlm_addr,
            pool_xlm_client,
            pool_usdc_addr,
            pool_eurc_addr,
            xlm_token_client,
            xlm_asset_client,
            usdc_token_client,
            usdc_asset_client,
            eurc_token_client,
            eurc_asset_client,
            reflector_addr,
            ..
        } = setup_test_env(&e);

        // Create loan for user2 to generate yield in pool for user3
        // which should result in user getting shares as collateral
        // of which amount does not equal amount of tokens.

        eurc_asset_client.mint(&user2, &100_000);
        usdc_asset_client.mint(&user2, &100_000);
        xlm_asset_client.mint(&admin, &100_000);
        xlm_asset_client.mint(&user2, &100_000);
        pool_xlm_client.deposit(&admin, &100_000);

        // Create a loan.
        let mut loan =
            manager_client.create_loan(&user2, &100, &pool_xlm_addr, &1000, &pool_usdc_addr);

        // Here borrowed amount should be the same as time has not moved. add_interest() is only called to store the LastUpdate sequence number.
        assert_eq!(loan.borrowed_amount, 100);
        assert_eq!(loan.health_factor, 80_000_000);
        assert_eq!(xlm_token_client.balance(&user2), 100_100);
        assert_eq!(usdc_token_client.balance(&user2), 99_000);

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
        assert_eq!(loan.collateral_amount, 1000);

        manager_client.repay_and_close_manager(&110, &loan.loan_id);

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
        assert_eq!(loan_usdc.collateral_amount, 99); // Note deposited 100 but as pool already has
                                                     // interest collateal is 99 shares
        assert_eq!(loan_usdc.borrowed_from, pool_usdc_addr);
        assert_eq!(loan_usdc.collateral_from, pool_xlm_addr);

        let loan_eurc = loans.get(1).unwrap();
        assert_eq!(loan_eurc.borrowed_amount, 30);
        assert_eq!(loan_eurc.collateral_amount, 299);
        assert_eq!(loan_eurc.borrowed_from, pool_eurc_addr);
        assert_eq!(loan_eurc.collateral_from, pool_xlm_addr);
    }

    #[test]
    fn repay_loan_with_shares() {
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
            admin,
            user,
            user2,
            manager_client,
            pool_xlm_addr,
            pool_xlm_client,
            pool_usdc_addr,
            pool_usdc_client,
            xlm_token_client,
            xlm_asset_client,
            usdc_token_client,
            usdc_asset_client,
            eurc_asset_client,
            reflector_addr,
            ..
        } = setup_test_env(&e);

        // Create loan for user2 to generate yield in pool for user3
        // which should result in user getting shares as collateral
        // of which amount does not equal amount of tokens.

        eurc_asset_client.mint(&user2, &1_000);
        usdc_asset_client.mint(&user2, &1_000);
        xlm_asset_client.mint(&admin, &100_000);
        xlm_asset_client.mint(&user2, &1_000);
        pool_xlm_client.deposit(&admin, &100_000);

        // Create a loan.
        let mut loan =
            manager_client.create_loan(&user2, &100, &pool_xlm_addr, &1000, &pool_usdc_addr);

        // Here borrowed amount should be the same as time has not moved. add_interest() is only called to store the LastUpdate sequence number.
        assert_eq!(loan.borrowed_amount, 100);
        assert_eq!(loan.health_factor, 80_000_000);
        assert_eq!(xlm_token_client.balance(&user2), 1100);
        assert_eq!(usdc_token_client.balance(&user2), 0);

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
        assert_eq!(loan.collateral_amount, 1000);

        manager_client.repay_and_close_manager(&110, &loan.loan_id);

        // ACT
        // Create a loan.
        usdc_asset_client.mint(&user, &900);
        xlm_asset_client.mint(&user, &2000);

        assert_eq!(100002, pool_xlm_client.get_available_balance());
        assert_eq!(100002, pool_xlm_client.get_contract_balance());
        assert_eq!(100000, pool_xlm_client.get_total_balance_shares());
        assert_eq!(3000, xlm_asset_client.balance(&user));
        assert_eq!(100000, pool_usdc_client.get_available_balance());

        let loan = manager_client.create_loan(&user, &900, &pool_usdc_addr, &3000, &pool_xlm_addr);

        assert_eq!(2999, loan.collateral_amount);

        usdc_asset_client.mint(&user2, &100_000);
        // Create a loan.
        let mut loan2 =
            manager_client.create_loan(&user2, &10_000, &pool_xlm_addr, &100_000, &pool_usdc_addr);

        // Here borrowed amount should be the same as time has not moved. add_interest() is only called to store the LastUpdate sequence number.
        assert_eq!(loan2.borrowed_amount, 10_000);
        assert_eq!(loan2.health_factor, 80_000_000);
        assert_eq!(xlm_token_client.balance(&user2), 10998);
        assert_eq!(usdc_token_client.balance(&user2), 1000);

        // Move time
        e.ledger().with_mut(|li| {
            li.sequence_number = 200_000 + 100_000;
            li.timestamp = 1 + 31_556_926 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        e.register_at(&reflector_addr, oracle::WASM, ());

        loan2 = manager_client.add_interest(&loan2.loan_id);

        assert_eq!(loan2.borrowed_amount, 10286);
        assert_eq!(loan2.health_factor, 77_775_617);
        assert_eq!(loan2.collateral_amount, 100_000);

        xlm_asset_client.mint(&user2, &10_000);
        manager_client.repay_and_close_manager(&11_000, &loan2.loan_id);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 300_000 + 100_000;
            li.timestamp = 1 + 31_556_926 + 31_556_926 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        e.register_at(&reflector_addr, oracle::WASM, ());

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 0);
        assert_eq!(usdc_token_client.balance(&user), 1800);

        let loans = manager_client.get_loans(&user);
        assert_eq!(loans.len(), 1);

        // mint the user some money so they can repay.
        usdc_asset_client.mint(&user, &200);
        manager_client.repay_and_close_manager(&1200, &loan.loan_id);
        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 200_000 + 1;
            li.timestamp = 1 + 31_556_926 + 31_556_926 + 1;
        });
        let loans = manager_client.get_loans(&user);
        assert_eq!(loans.len(), 0);
        assert_eq!(100034, pool_usdc_client.get_available_balance());
        assert_eq!(100034, pool_usdc_client.get_contract_balance());
        assert_eq!(100000, pool_usdc_client.get_total_balance_shares());
        assert_eq!(100254, pool_xlm_client.get_available_balance());
        assert_eq!(100254, pool_xlm_client.get_contract_balance());
        assert_eq!(100000, pool_xlm_client.get_total_balance_shares());
        assert_eq!(3006, xlm_asset_client.balance(&user)); // xlm balance grew from 3000 -> 3006
                                                           // while being collateral
    }

    #[test]
    fn liquidate_loan_with_shares() {}

    /* Test setup helpers */
    struct TestEnv<'a> {
        admin: Address,
        user: Address,
        user2: Address,
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
        let user2 = Address::generate(e);
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
        pool_usdc_client.deposit(&admin, &100_000);
        pool_eurc_client.deposit(&admin, &100_000);

        TestEnv {
            admin,
            user,
            user2,
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
        const LIQUIDATION_THRESHOLD: i128 = 8_000_000; // 80%
        let wasm_hash = e.deployer().upload_contract_wasm(loan_pool::WASM);
        let xdr_bytes = token_address.clone().to_xdr(e);
        let salt = e.crypto().sha256(&xdr_bytes).to_bytes();
        manager_client.deploy_pool(
            &wasm_hash,
            &salt,
            token_address,
            ticker,
            &LIQUIDATION_THRESHOLD,
        )
    }
}
