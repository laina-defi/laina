use crate::error::LoanManagerError;
use crate::oracle::{self, Asset};
use crate::storage::{self, Loan};

use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, String, Symbol};

mod loan_pool {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/loan_pool.wasm");
}

// This is the real address of the Reflector Oracle in Testnet.
// We use the same address to mock it for testing.
const REFLECTOR_ADDRESS: &str = "CCYOZJCOPG34LLQQ7N24YXBM7LL62R7ONMZ3G6WZAAYPB5OYKOMJRN63";

#[contract]
struct LoanManager;

#[allow(dead_code)]
#[contractimpl]
impl LoanManager {
    /// Set the admin that's allowed to upgrade the wasm.
    pub fn initialize(e: Env, admin: Address) -> Result<(), LoanManagerError> {
        if storage::admin_exists(&e) {
            return Err(LoanManagerError::AlreadyInitialized);
        }
        storage::write_admin(&e, &admin);

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
    ) -> Result<(), LoanManagerError> {
        user.require_auth();

        if storage::loan_exists(&e, user.clone()) {
            return Err(LoanManagerError::LoanAlreadyExists);
        }

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

        // Health factor has to be over 1.2 for the loan to be initialized.
        // Health factor is defined as so: 1.0 = 10000000_i128
        const HEALTH_FACTOR_THRESHOLD: i128 = 10000000;
        assert!(
            health_factor > HEALTH_FACTOR_THRESHOLD,
            "Health factor must be over {HEALTH_FACTOR_THRESHOLD} to create a new loan!"
        );

        // Deposit collateral
        let collateral_amount = collateral_pool_client.deposit_collateral(&user, &collateral);

        // Borrow the funds
        let borrowed_amount = borrow_pool_client.borrow(&user, &borrowed);

        let unpaid_interest = 0;

        let loan = Loan {
            borrower: user.clone(),
            borrowed_amount,
            borrowed_from,
            collateral_amount,
            collateral_from,
            health_factor,
            unpaid_interest,
            last_accrual: borrow_pool_client.get_accrual(),
        };

        storage::write_loan(&e, user.clone(), loan);

        Ok(())
    }

    pub fn add_interest(e: &Env, user: Address) -> Result<(), LoanManagerError> {
        const DECIMAL: i128 = 10000000;
        let Loan {
            borrower,
            borrowed_from,
            collateral_amount,
            borrowed_amount,
            collateral_from,
            unpaid_interest,
            last_accrual,
            ..
        } = storage::read_loan(e, user.clone()).ok_or(LoanManagerError::LoanNotFound)?;

        let borrow_pool_client = loan_pool::Client::new(e, &borrowed_from);
        let collateral_pool_client = loan_pool::Client::new(e, &collateral_from);

        let token_ticker = borrow_pool_client.get_currency().ticker;

        let token_collateral_ticker = collateral_pool_client.get_currency().ticker;

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

        let new_health_factor = Self::calculate_health_factor(
            e,
            token_ticker,
            new_borrowed_amount,
            token_collateral_ticker,
            collateral_amount,
            collateral_from.clone(),
        )?;

        let borrow_change = new_borrowed_amount
            .checked_sub(borrowed_amount)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;
        let new_unpaid_interest = unpaid_interest
            .checked_add(borrow_change)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let updated_loan = Loan {
            borrower,
            borrowed_from,
            collateral_amount,
            borrowed_amount: new_borrowed_amount,
            collateral_from,
            health_factor: new_health_factor,
            unpaid_interest: new_unpaid_interest,
            last_accrual: current_accrual,
        };

        storage::write_loan(e, user.clone(), updated_loan.clone());

        Ok(())
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
        let reflector_address = Address::from_string(&String::from_str(e, REFLECTOR_ADDRESS));
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

    pub fn get_loan(e: &Env, user: Address) -> Result<Loan, LoanManagerError> {
        storage::read_loan(e, user).ok_or(LoanManagerError::LoanNotFound)
    }

    pub fn get_price(e: &Env, token: Symbol) -> Result<i128, LoanManagerError> {
        let reflector_address = Address::from_string(&String::from_str(e, REFLECTOR_ADDRESS));
        let reflector_contract = oracle::Client::new(e, &reflector_address);

        let asset = Asset::Other(token);

        let asset_pricedata = reflector_contract
            .lastprice(&asset)
            .ok_or(LoanManagerError::NoLastPrice)?;
        Ok(asset_pricedata.price)
    }

    pub fn repay(e: &Env, user: Address, amount: i128) -> Result<(i128, i128), LoanManagerError> {
        user.require_auth();

        Self::add_interest(e, user.clone())?;

        let Loan {
            borrower,
            borrowed_amount,
            borrowed_from,
            collateral_amount,
            collateral_from,
            unpaid_interest,
            last_accrual,
            ..
        } = storage::read_loan(e, user.clone()).ok_or(LoanManagerError::LoanNotFound)?;

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

        let new_health_factor = Self::calculate_health_factor(
            e,
            borrow_pool_client.get_currency().ticker,
            new_borrowed_amount,
            collateral_pool_client.get_currency().ticker,
            collateral_amount,
            collateral_from.clone(),
        )?;

        let loan = Loan {
            borrower,
            borrowed_amount: new_borrowed_amount,
            borrowed_from,
            collateral_amount,
            collateral_from,
            health_factor: new_health_factor,
            unpaid_interest: new_unpaid_interest,
            last_accrual,
        };

        storage::write_loan(e, user, loan);

        Ok((borrowed_amount, new_borrowed_amount))
    }

    pub fn repay_and_close_manager(
        e: &Env,
        user: Address,
        max_allowed_amount: i128,
    ) -> Result<i128, LoanManagerError> {
        user.require_auth();

        Self::add_interest(e, user.clone())?;

        let Loan {
            borrowed_amount,
            borrowed_from,
            collateral_amount,
            collateral_from,
            unpaid_interest,
            ..
        } = storage::read_loan(e, user.clone()).ok_or(LoanManagerError::LoanNotFound)?;

        let borrow_pool_client = loan_pool::Client::new(e, &borrowed_from);
        borrow_pool_client.repay_and_close(
            &user,
            &borrowed_amount,
            &max_allowed_amount,
            &unpaid_interest,
        );

        let collateral_pool_client = loan_pool::Client::new(e, &collateral_from);
        collateral_pool_client.withdraw_collateral(&user, &collateral_amount);

        storage::delete_loan(e, user);
        Ok(borrowed_amount)
    }

    pub fn liquidate(
        e: Env,
        user: Address,
        borrower: Address,
        amount: i128,
    ) -> Result<(i128, i128), LoanManagerError> {
        user.require_auth();

        Self::add_interest(&e, borrower.clone())?;

        let Loan {
            borrower,
            borrowed_amount,
            borrowed_from,
            collateral_from,
            collateral_amount,
            unpaid_interest,
            last_accrual,
            ..
        } = storage::read_loan(&e, borrower.clone()).ok_or(LoanManagerError::LoanNotFound)?;

        let borrow_pool_client = loan_pool::Client::new(&e, &borrowed_from);
        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_from);

        let borrowed_ticker = borrow_pool_client.get_currency().ticker;
        let collateral_ticker = collateral_pool_client.get_currency().ticker;

        // Check that loan is for sure liquidatable at this moment.
        assert!(
            Self::calculate_health_factor(
                &e,
                borrowed_ticker.clone(),
                borrowed_amount,
                collateral_ticker.clone(),
                collateral_amount,
                collateral_from.clone(),
            )? < 10000000
        ); // Temp high value for testing
        assert!(
            amount
                < (borrowed_amount
                    .checked_div(2)
                    .ok_or(LoanManagerError::OverOrUnderFlow)?)
        );

        let borrowed_price = Self::get_price(&e, borrowed_ticker.clone())?;
        let collateral_price = Self::get_price(&e, collateral_ticker.clone())?;

        const TEMP_BONUS: i128 = 10_500_000; // multiplier 1.05 -> 5%

        let liquidation_value = amount
            .checked_mul(borrowed_price)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;
        let collateral_amount_bonus = liquidation_value
            .checked_mul(TEMP_BONUS)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(collateral_price)
            .ok_or(LoanManagerError::OverOrUnderFlow)?
            .checked_div(10_000_000)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        borrow_pool_client.liquidate(&user, &amount, &unpaid_interest, &borrower);

        collateral_pool_client.liquidate_transfer_collateral(
            &user,
            &collateral_amount_bonus,
            &borrower,
        );

        let new_borrowed_amount = borrowed_amount
            .checked_sub(amount)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;
        let new_collateral_amount = collateral_amount
            .checked_sub(collateral_amount_bonus)
            .ok_or(LoanManagerError::OverOrUnderFlow)?;

        let new_health_factor = Self::calculate_health_factor(
            &e,
            borrowed_ticker,
            new_borrowed_amount,
            collateral_ticker,
            new_collateral_amount,
            collateral_from.clone(),
        )?;

        let new_loan = Loan {
            borrower: borrower.clone(),
            borrowed_amount: new_borrowed_amount,
            borrowed_from,
            collateral_from,
            collateral_amount: new_collateral_amount,
            health_factor: new_health_factor,
            unpaid_interest, // Temp
            last_accrual,
        };

        storage::write_loan(&e, borrower, new_loan);

        Ok((new_borrowed_amount, new_collateral_amount))
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
        let manager_addr = e.register(LoanManager, ());
        let manager_client = LoanManagerClient::new(&e, &manager_addr);

        assert!(manager_client.try_initialize(&admin).is_ok());
    }

    #[test]
    fn cannot_re_initialize() {
        let e = Env::default();
        let admin = Address::generate(&e);

        let contract_id = e.register(LoanManager, ());
        let client = LoanManagerClient::new(&e, &contract_id);

        client.initialize(&admin);

        assert!(client.try_initialize(&admin).is_err())
    }

    #[test]
    fn deploy_pool() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths();

        // ACT
        // Deploy contract using loan_manager as factory
        let TestEnv {
            pool_xlm_client, ..
        } = setup_test_env(&e);

        // ASSERT
        // No authorizations needed - the contract acts as a factory.
        // assert_eq!(e.auths(), &[]);

        // Invoke contract to check that it is initialized.
        let pool_balance = pool_xlm_client.get_contract_balance();
        assert_eq!(pool_balance, 1000);
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
        } = setup_test_env(&e);

        xlm_asset_client.mint(&admin, &9_001);
        pool_xlm_client.deposit(&admin, &9_001);
        usdc_asset_client.mint(&user, &100_000);

        // Create a loan.
        manager_client.create_loan(&user, &1_000, &pool_xlm_addr, &100_000, &pool_usdc_addr);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 1_000);
        assert_eq!(usdc_token_client.balance(&user), 1_000);

        let mut user_loan = manager_client.get_loan(&user);

        assert_eq!(user_loan.borrowed_amount, 1_000);
        assert_eq!(user_loan.collateral_amount, 100_000);

        manager_client.repay(&user, &100);
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000 + 1;
        });

        user_loan = manager_client.get_loan(&user);

        assert_eq!(user_loan.borrowed_amount, 928);
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
            pool_xlm_client,
            xlm_asset_client,
            ..
        } = setup_test_env(&e);

        // ACT
        // Create a loan.
        manager_client.create_loan(&user, &100, &pool_xlm_addr, &500, &pool_usdc_addr);
        assert_eq!(pool_xlm_client.get_available_balance(), 900);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 100);
        assert_eq!(usdc_token_client.balance(&user), 500);

        let user_loan = manager_client.get_loan(&user);

        assert_eq!(user_loan.borrowed_amount, 100);
        assert_eq!(user_loan.collateral_amount, 500);

        manager_client.repay(&user, &50);
        let user_loan = manager_client.get_loan(&user);
        assert_eq!(user_loan.borrowed_amount, 52);

        assert_eq!((52, 2), manager_client.repay(&user, &50));
        assert_eq!(1000, pool_xlm_client.get_available_balance());
        assert_eq!(1002, pool_xlm_client.get_contract_balance());
        assert_eq!(1000, pool_xlm_client.get_total_balance_shares());

        // Create a new user that should not be able to withdraw more than what they have deposited even if the pool already has interest
        let new_user = Address::generate(&e);
        xlm_asset_client.mint(&new_user, &1_000);

        pool_xlm_client.deposit(&new_user, &1000);
        assert_eq!(2002, pool_xlm_client.get_contract_balance());
        let positions_new_user = Positions {
            collateral: 0,
            liabilities: 0,
            receivable_shares: 998,
        };
        assert_eq!(
            positions_new_user,
            pool_xlm_client.get_user_positions(&new_user)
        );

        let test_positions_admin = Positions {
            collateral: 0,
            liabilities: 0,
            receivable_shares: 1000,
        };
        assert_eq!(
            test_positions_admin,
            pool_xlm_client.get_user_positions(&admin)
        );
        let pool_state = PoolState {
            annual_interest_rate: 200887,
            available_balance_tokens: 2000,
            total_balance_shares: 1998,
            total_balance_tokens: 2002,
        };
        assert_eq!(pool_state, pool_xlm_client.get_pool_state());

        let state_after_first_withdraw = pool_xlm_client.withdraw(&new_user, &1002);
        assert_eq!(state_after_first_withdraw, pool_xlm_client.get_pool_state());

        let pool_state = PoolState {
            annual_interest_rate: 200887,
            available_balance_tokens: 998,
            total_balance_shares: 998,
            total_balance_tokens: 1000,
        };
        assert_eq!(pool_state, pool_xlm_client.get_pool_state());
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
            xlm_token_client,
            usdc_token_client,
            ..
        } = setup_test_env(&e);

        // ACT
        manager_client.create_loan(&user, &10, &pool_xlm_addr, &100, &pool_usdc_addr);

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 10);
        assert_eq!(usdc_token_client.balance(&user), 900);
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
            usdc_token_client,
            ..
        } = setup_test_env(&e);

        // ACT

        // Create a loan.
        manager_client.create_loan(&user, &100, &pool_xlm_addr, &1000, &pool_usdc_addr);

        let user_loan = manager_client.get_loan(&user);

        // Here borrowed amount should be the same as time has not moved. add_interest() is only called to store the LastUpdate sequence number.
        assert_eq!(user_loan.borrowed_amount, 100);
        assert_eq!(user_loan.health_factor, 80_000_000);
        assert_eq!(usdc_token_client.balance(&user), 0);

        // Move time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        manager_client.add_interest(&user);

        let user_loan = manager_client.get_loan(&user);

        assert_eq!(user_loan.borrowed_amount, 102);
        assert_eq!(user_loan.health_factor, 78_431_372);
        assert_eq!(user_loan.collateral_amount, 1000);
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
            pool_xlm_client,
            xlm_token_client,
            usdc_token_client,
            ..
        } = setup_test_env(&e);

        // ACT
        // Create a loan.
        manager_client.create_loan(&user, &100, &pool_xlm_addr, &500, &pool_usdc_addr);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // ASSERT
        assert_eq!(xlm_token_client.balance(&user), 100);
        assert_eq!(usdc_token_client.balance(&user), 500);

        let user_loan = manager_client.get_loan(&user);

        assert_eq!(user_loan.borrowed_amount, 100);
        assert_eq!(user_loan.collateral_amount, 500);

        manager_client.repay(&user, &50);
        let user_loan = manager_client.get_loan(&user);
        assert_eq!(user_loan.borrowed_amount, 52);

        assert_eq!((52, 2), manager_client.repay(&user, &50));
        assert_eq!(1000, pool_xlm_client.get_available_balance());
        assert_eq!(1002, pool_xlm_client.get_contract_balance());
        assert_eq!(1000, pool_xlm_client.get_total_balance_shares());
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
            xlm_asset_client,
            pool_xlm_client,
            pool_xlm_addr,
            pool_usdc_addr,
            xlm_token_client,
            usdc_token_client,
            ..
        } = setup_test_env(&e);

        // ACT
        // Create a loan.
        manager_client.create_loan(&user, &100, &pool_xlm_addr, &300, &pool_usdc_addr);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // ASSERT
        // A new instance of reflector mock needs to be created, they only live for one ledger.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        assert_eq!(xlm_token_client.balance(&user), 100);
        assert_eq!(usdc_token_client.balance(&user), 700);

        let user_loan = manager_client.get_loan(&user);

        assert_eq!(user_loan.borrowed_amount, 100);
        assert_eq!(user_loan.collateral_amount, 300);

        // mint the user some money so they can repay.
        xlm_asset_client.mint(&user, &45);
        assert_eq!(
            102,
            manager_client.repay_and_close_manager(&user, &(user_loan.borrowed_amount + 45))
        );

        assert_eq!(1002, pool_xlm_client.get_available_balance());
        assert_eq!(1002, pool_xlm_client.get_contract_balance());
        assert_eq!(1000, pool_xlm_client.get_total_balance_shares());
        assert_eq!(1000, usdc_token_client.balance(&user));
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
        manager_client.create_loan(&user, &100, &pool_xlm_addr, &1000, &pool_usdc_addr);

        manager_client.repay(&user, &2_000);
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
            pool_xlm_client,
            pool_usdc_addr,
            xlm_asset_client,
            usdc_asset_client,
            ..
        } = setup_test_env(&e);

        // print more money
        xlm_asset_client.mint(&admin, &9_001);
        pool_xlm_client.deposit(&admin, &9_001);
        usdc_asset_client.mint(&user, &12_000);

        // ACT
        // Create a loan.
        manager_client.create_loan(&user, &10_000, &pool_xlm_addr, &12_505, &pool_usdc_addr);

        let user_loan = manager_client.get_loan(&user);

        assert_eq!(user_loan.borrowed_amount, 10_000);

        manager_client.add_interest(&user);

        // Here borrowed amount should be the same as time has not moved. add_interest() is only called to store the LastUpdate sequence number.
        assert_eq!(user_loan.borrowed_amount, 10_000);
        assert_eq!(user_loan.health_factor, 10_004_000);

        // Move time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        manager_client.add_interest(&user);

        let user_loan = manager_client.get_loan(&user);

        assert_eq!(user_loan.borrowed_amount, 12_998);
        assert_eq!(user_loan.health_factor, 7_696_568);
        assert_eq!(user_loan.collateral_amount, 12_505);

        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 1_000;
        });

        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        manager_client.liquidate(&admin, &user, &5_000);

        let user_loan = manager_client.get_loan(&user);

        assert_eq!(user_loan.borrowed_amount, 7_998);
        assert_eq!(user_loan.health_factor, 7_256_814);
        assert_eq!(user_loan.collateral_amount, 7_255);
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
    }

    fn setup_test_env(e: &Env) -> TestEnv {
        let admin = Address::generate(e);
        let admin2 = Address::generate(e);
        let user = Address::generate(e);

        // loan manager
        let manager_addr = e.register(LoanManager, ());
        let manager_client = LoanManagerClient::new(e, &manager_addr);
        manager_client.initialize(&admin);

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

        // Mint the admin and the user some coins
        xlm_asset_client.mint(&admin, &1_000_000);
        usdc_asset_client.mint(&user, &1_000);

        // Setup mock price oracle
        let reflector_addr = Address::from_string(&String::from_str(e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // Deposit some of the admin's tokens for borrowing.
        pool_xlm_client.deposit(&admin, &1_000);

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
