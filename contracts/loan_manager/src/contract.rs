use crate::error::LoanManagerError;
use crate::oracle::{self, Asset};
use crate::storage::{
    self, Loan, LoanManagerDataKey, POSITIONS_BUMP_AMOUNT, POSITIONS_LIFETIME_THRESHOLD,
};

use soroban_sdk::{contract, contractimpl, symbol_short, Address, BytesN, Env, String, Symbol};

mod loan_pool {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32-unknown-unknown/release/loan_pool.wasm"
    );
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

        // Return the contract ID of the deployed contract
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

        if e.storage()
            .persistent()
            .has(&LoanManagerDataKey::Loan(user.clone()))
        {
            return Err(LoanManagerError::LoanAlreadyExists);
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
            health_factor: _,
            unpaid_interest,
            last_accrual,
        } = storage::read_loan(e, user.clone()).ok_or(LoanManagerError::LoanNotFound)?;

        let borrow_pool_client = loan_pool::Client::new(e, &borrowed_from);
        let collateral_pool_client = loan_pool::Client::new(e, &collateral_from);

        let loan_pool::Currency {
            ticker: token_ticker,
            ..
        } = borrow_pool_client.get_currency();

        let loan_pool::Currency {
            ticker: token_collateral_ticker,
            ..
        } = collateral_pool_client.get_currency();

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

        let key = (Symbol::new(e, "Loan"), user.clone());
        e.events()
            .publish((key, symbol_short!("updated")), updated_loan);

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

    pub fn get_loan(e: &Env, user: Address) -> Option<Loan> {
        storage::read_loan(e, user)
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

        let key = (Symbol::new(e, "Loan"), user.clone());
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

        e.storage().persistent().set(&key, &loan);
        e.storage().persistent().extend_ttl(
            &key,
            POSITIONS_LIFETIME_THRESHOLD,
            POSITIONS_BUMP_AMOUNT,
        );
        e.events().publish((key, symbol_short!("updated")), loan);

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
            borrower: _,
            borrowed_amount,
            borrowed_from,
            collateral_amount,
            collateral_from,
            health_factor: _,
            unpaid_interest,
            last_accrual: _,
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

        let key = (Symbol::new(e, "Loan"), user.clone());
        e.storage().persistent().remove(&key);
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
            health_factor: _,
            unpaid_interest,
            last_accrual,
        } = storage::read_loan(&e, borrower.clone()).ok_or(LoanManagerError::LoanNotFound)?;

        let key = (Symbol::new(&e, "Loan"), borrower.clone());

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
            borrower,
            borrowed_amount: new_borrowed_amount,
            borrowed_from,
            collateral_from,
            collateral_amount: new_collateral_amount,
            health_factor: new_health_factor,
            unpaid_interest, // Temp
            last_accrual,
        };

        e.storage().persistent().set(&key, &new_loan);

        Ok((new_borrowed_amount, new_collateral_amount))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::{Client as TokenClient, StellarAssetClient},
        Env,
    };
    mod loan_manager {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32-unknown-unknown/release/loan_manager.wasm"
        );
    }

    #[test]
    fn initialize() {
        let e = Env::default();
        let admin = Address::generate(&e);

        let contract_id = e.register(LoanManager, ());
        let client = LoanManagerClient::new(&e, &contract_id);

        assert!(client.try_initialize(&admin).is_ok());
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

        let admin = Address::generate(&e);
        let deployer_client = LoanManagerClient::new(&e, &e.register(LoanManager, ()));
        deployer_client.initialize(&admin);

        // Setup test token
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let ticker = Symbol::new(&e, "XLM");

        let wasm_hash = e.deployer().upload_contract_wasm(loan_pool::WASM);
        let salt = BytesN::from_array(&e, &[0; 32]);

        // ACT
        // Deploy contract using loan_manager as factory
        let loan_pool_addr =
            deployer_client.deploy_pool(&wasm_hash, &salt, &token.address(), &ticker, &8_000_000);

        // ASSERT
        // No authorizations needed - the contract acts as a factory.
        // assert_eq!(e.auths(), &[]);

        // Invoke contract to check that it is initialized.
        let loan_pool_client = loan_pool::Client::new(&e, &loan_pool_addr);
        let pool_balance = loan_pool_client.get_contract_balance();
        assert_eq!(pool_balance, 0);
    }

    #[test]
    fn upgrade_manager_and_pool() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);

        let deployer_client = LoanManagerClient::new(&e, &e.register(LoanManager, ()));
        deployer_client.initialize(&admin);

        // Setup test token
        let token = e.register_stellar_asset_contract_v2(admin.clone());
        let ticker = Symbol::new(&e, "XLM");

        let manager_wasm_hash = e.deployer().upload_contract_wasm(loan_manager::WASM);
        let pool_wasm_hash = e.deployer().upload_contract_wasm(loan_pool::WASM);
        let salt = BytesN::from_array(&e, &[0; 32]);

        // ACT
        deployer_client.deploy_pool(
            &pool_wasm_hash,
            &salt,
            &token.address(),
            &ticker,
            &8_000_000,
        );
        deployer_client.upgrade(&manager_wasm_hash, &pool_wasm_hash);
    }

    #[test]
    fn create_loan() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&e);
        let loan_token = e.register_stellar_asset_contract_v2(admin.clone());
        let loan_asset = StellarAssetClient::new(&e, &loan_token.address());
        let loan_token_client = TokenClient::new(&e, &loan_token.address());
        loan_asset.mint(&admin, &1000);
        let loan_currency = loan_pool::Currency {
            token_address: loan_token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let admin2 = Address::generate(&e);
        let collateral_token = e.register_stellar_asset_contract_v2(admin2.clone());
        let collateral_asset = StellarAssetClient::new(&e, &collateral_token.address());
        let collateral_token_client = TokenClient::new(&e, &collateral_token.address());
        let collateral_currency = loan_pool::Currency {
            token_address: collateral_token.address(),
            ticker: Symbol::new(&e, "USDC"),
        };

        // Register mock Reflector contract.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // Mint the user some coins
        let user = Address::generate(&e);
        collateral_asset.mint(&user, &1000);

        assert_eq!(collateral_token_client.balance(&user), 1000);

        // Set up a loan pool with funds for borrowing.
        let loan_pool_id = e.register(loan_pool::WASM, ());
        let loan_pool_client = loan_pool::Client::new(&e, &loan_pool_id);

        // Set up a loan_pool for the collaterals.
        let collateral_pool_id = e.register(loan_pool::WASM, ());
        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_pool_id);

        // Register loan manager contract.
        let contract_id = e.register(LoanManager, ());
        let contract_client = LoanManagerClient::new(&e, &contract_id);

        // ACT
        // Initialize the loan pool and deposit some of the admin's funds.
        loan_pool_client.initialize(&contract_id, &loan_currency, &8_000_000);
        loan_pool_client.deposit(&admin, &1000);

        collateral_pool_client.initialize(&contract_id, &collateral_currency, &8_000_000);

        contract_client.create_loan(&user, &10, &loan_pool_id, &100, &collateral_pool_id);

        // ASSERT
        assert_eq!(loan_token_client.balance(&user), 10);
        assert_eq!(collateral_token_client.balance(&user), 900);
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

        let admin = Address::generate(&e);
        let loan_token = e.register_stellar_asset_contract_v2(admin.clone());
        let loan_asset = StellarAssetClient::new(&e, &loan_token.address());
        loan_asset.mint(&admin, &1_000_000);
        let loan_currency = loan_pool::Currency {
            token_address: loan_token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let admin2 = Address::generate(&e);
        let collateral_token = e.register_stellar_asset_contract_v2(admin2.clone());
        let collateral_asset = StellarAssetClient::new(&e, &collateral_token.address());
        let collateral_token_client = TokenClient::new(&e, &collateral_token.address());
        let collateral_currency = loan_pool::Currency {
            token_address: collateral_token.address(),
            ticker: Symbol::new(&e, "USDC"),
        };

        // Register mock Reflector contract.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // Mint the user some coins
        let user = Address::generate(&e);
        collateral_asset.mint(&user, &1_000_000);

        assert_eq!(collateral_token_client.balance(&user), 1_000_000);

        // Set up a loan pool with funds for borrowing.
        let loan_pool_id = e.register(loan_pool::WASM, ());
        let loan_pool_client = loan_pool::Client::new(&e, &loan_pool_id);

        // Set up a loan_pool for the collaterals.
        let collateral_pool_id = e.register(loan_pool::WASM, ());
        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_pool_id);

        // Register loan manager contract.
        let contract_id = e.register(LoanManager, ());
        let contract_client = LoanManagerClient::new(&e, &contract_id);

        // ACT
        // Initialize the loan pool and deposit some of the admin's funds.
        loan_pool_client.initialize(&contract_id, &loan_currency, &8_000_000);
        loan_pool_client.deposit(&admin, &10_001);

        collateral_pool_client.initialize(&contract_id, &collateral_currency, &8_000_000);

        // Create a loan.
        contract_client.create_loan(&user, &10_000, &loan_pool_id, &100_000, &collateral_pool_id);

        let user_loan = contract_client.get_loan(&user).unwrap();

        assert_eq!(user_loan.borrowed_amount, 10_000);
        assert_eq!(collateral_token_client.balance(&user), 900_000);

        // Here borrowed amount should be the same as time has not moved. add_interest() is only called to store the LastUpdate sequence number.
        assert_eq!(user_loan.borrowed_amount, 10_000);
        assert_eq!(user_loan.health_factor, 80_000_000);
        assert_eq!(collateral_token_client.balance(&user), 900_000);

        // Move time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // A new instance of reflector mock needs to be created, they only live for one ledger.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        contract_client.add_interest(&user);

        let user_loan = contract_client.get_loan(&user).unwrap();

        assert_eq!(user_loan.borrowed_amount, 12_998);
        assert_eq!(user_loan.health_factor, 61_547_930);
        assert_eq!(user_loan.collateral_amount, 100_000);
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

        let admin = Address::generate(&e);
        let loan_token = e.register_stellar_asset_contract_v2(admin.clone());
        let loan_asset = StellarAssetClient::new(&e, &loan_token.address());
        let loan_token_client = TokenClient::new(&e, &loan_token.address());
        loan_asset.mint(&admin, &1_000_000);
        let loan_currency = loan_pool::Currency {
            token_address: loan_token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let admin2 = Address::generate(&e);
        let collateral_token = e.register_stellar_asset_contract_v2(admin2.clone());
        let collateral_asset = StellarAssetClient::new(&e, &collateral_token.address());
        let collateral_token_client = TokenClient::new(&e, &collateral_token.address());
        let collateral_currency = loan_pool::Currency {
            token_address: collateral_token.address(),
            ticker: Symbol::new(&e, "USDC"),
        };

        // Register mock Reflector contract.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // Mint the user some coins
        let user = Address::generate(&e);
        collateral_asset.mint(&user, &1_000_000);

        assert_eq!(collateral_token_client.balance(&user), 1_000_000);

        // Set up a loan pool with funds for borrowing.
        let loan_pool_id = e.register(loan_pool::WASM, ());
        let loan_pool_client = loan_pool::Client::new(&e, &loan_pool_id);

        // Set up a loan_pool for the collaterals.
        let collateral_pool_id = e.register(loan_pool::WASM, ());
        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_pool_id);

        // Register loan manager contract.
        let contract_id = e.register(LoanManager, ());
        let contract_client = LoanManagerClient::new(&e, &contract_id);

        // ACT
        // Initialize the loan pool and deposit some of the admin's funds.
        loan_pool_client.initialize(&contract_id, &loan_currency, &8_000_000);
        loan_pool_client.deposit(&admin, &1_000_000);

        collateral_pool_client.initialize(&contract_id, &collateral_currency, &8_000_000);

        // Create a loan.
        contract_client.create_loan(&user, &1_000, &loan_pool_id, &100_000, &collateral_pool_id);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // ASSERT
        assert_eq!(loan_token_client.balance(&user), 1_000);
        assert_eq!(collateral_token_client.balance(&user), 900_000);

        let user_loan = contract_client.get_loan(&user).unwrap();

        assert_eq!(user_loan.borrowed_amount, 1_000);
        assert_eq!(user_loan.collateral_amount, 100_000);

        contract_client.repay(&user, &100);
        let user_loan = contract_client.get_loan(&user).unwrap();
        assert_eq!(user_loan.borrowed_amount, 920);

        assert_eq!((920, 820), contract_client.repay(&user, &100));
        assert_eq!(999198, loan_pool_client.get_available_balance());
        assert_eq!(1000018, loan_pool_client.get_contract_balance());
        assert_eq!(1000000, loan_pool_client.get_total_balance_shares());
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

        let admin = Address::generate(&e);
        let loan_token = e.register_stellar_asset_contract_v2(admin.clone());
        let loan_asset = StellarAssetClient::new(&e, &loan_token.address());
        let loan_token_client = TokenClient::new(&e, &loan_token.address());
        loan_asset.mint(&admin, &1_000_000);
        let loan_currency = loan_pool::Currency {
            token_address: loan_token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let admin2 = Address::generate(&e);
        let collateral_token = e.register_stellar_asset_contract_v2(admin2.clone());
        let collateral_asset = StellarAssetClient::new(&e, &collateral_token.address());
        let collateral_token_client = TokenClient::new(&e, &collateral_token.address());
        let collateral_currency = loan_pool::Currency {
            token_address: collateral_token.address(),
            ticker: Symbol::new(&e, "USDC"),
        };

        // Register mock Reflector contract.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // Mint the user some coins
        let user = Address::generate(&e);
        loan_asset.mint(&user, &50);
        collateral_asset.mint(&user, &1_000_000);

        assert_eq!(collateral_token_client.balance(&user), 1_000_000);

        // Set up a loan pool with funds for borrowing.
        let loan_pool_id = e.register(loan_pool::WASM, ());
        let loan_pool_client = loan_pool::Client::new(&e, &loan_pool_id);

        // Set up a loan_pool for the collaterals.
        let collateral_pool_id = e.register(loan_pool::WASM, ());
        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_pool_id);

        // Register loan manager contract.
        let contract_id = e.register(LoanManager, ());
        let contract_client = LoanManagerClient::new(&e, &contract_id);

        // ACT
        // Initialize the loan pool and deposit some of the admin's funds.
        loan_pool_client.initialize(&contract_id, &loan_currency, &8_000_000);
        loan_pool_client.deposit(&admin, &1_000_000);

        collateral_pool_client.initialize(&contract_id, &collateral_currency, &8_000_000);

        // Create a loan.
        contract_client.create_loan(&user, &1_000, &loan_pool_id, &100_000, &collateral_pool_id);

        // Move in time
        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 100_000;
            li.timestamp = 1 + 31_556_926;
        });

        // ASSERT
        // A new instance of reflector mock needs to be created, they only live for one ledger.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        assert_eq!(loan_token_client.balance(&user), 1_050);
        assert_eq!(collateral_token_client.balance(&user), 900_000);

        let user_loan = contract_client.get_loan(&user).unwrap();

        assert_eq!(user_loan.borrowed_amount, 1_000);
        assert_eq!(user_loan.collateral_amount, 100_000);

        assert_eq!(
            1020,
            contract_client.repay_and_close_manager(&user, &(user_loan.borrowed_amount + 45))
        );

        assert_eq!(1000018, loan_pool_client.get_available_balance());
        assert_eq!(1000018, loan_pool_client.get_contract_balance());
        assert_eq!(1000000, loan_pool_client.get_total_balance_shares());
    }

    #[test]
    #[should_panic(expected = "Amount can not be greater than borrowed amount!")]
    fn repay_more_than_borrowed() {
        // ARRANGE
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&e);
        let loan_token = e.register_stellar_asset_contract_v2(admin.clone());
        let loan_asset = StellarAssetClient::new(&e, &loan_token.address());
        loan_asset.mint(&admin, &1_000_000);
        let loan_currency = loan_pool::Currency {
            token_address: loan_token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let admin2 = Address::generate(&e);
        let collateral_token = e.register_stellar_asset_contract_v2(admin2.clone());
        let collateral_asset = StellarAssetClient::new(&e, &collateral_token.address());
        let collateral_token_client = TokenClient::new(&e, &collateral_token.address());
        let collateral_currency = loan_pool::Currency {
            token_address: collateral_token.address(),
            ticker: Symbol::new(&e, "USDC"),
        };

        // Register mock Reflector contract.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // Mint the user some coins
        let user = Address::generate(&e);
        collateral_asset.mint(&user, &1_000_000);

        assert_eq!(collateral_token_client.balance(&user), 1_000_000);

        // Set up a loan pool with funds for borrowing.
        let loan_pool_id = e.register(loan_pool::WASM, ());
        let loan_pool_client = loan_pool::Client::new(&e, &loan_pool_id);

        // Set up a loan_pool for the collaterals.
        let collateral_pool_id = e.register(loan_pool::WASM, ());
        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_pool_id);

        // Register loan manager contract.
        let contract_id = e.register(LoanManager, ());
        let contract_client = LoanManagerClient::new(&e, &contract_id);

        // ACT
        // Initialize the loan pool and deposit some of the admin's funds.
        loan_pool_client.initialize(&contract_id, &loan_currency, &8_000_000);
        loan_pool_client.deposit(&admin, &1_000_000);

        collateral_pool_client.initialize(&contract_id, &collateral_currency, &8_000_000);

        // Create a loan.
        contract_client.create_loan(&user, &1_000, &loan_pool_id, &100_000, &collateral_pool_id);

        contract_client.repay(&user, &2_000);
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

        let admin = Address::generate(&e);
        let loan_token = e.register_stellar_asset_contract_v2(admin.clone());
        let loan_asset = StellarAssetClient::new(&e, &loan_token.address());
        loan_asset.mint(&admin, &1_000_000);
        let loan_currency = loan_pool::Currency {
            token_address: loan_token.address(),
            ticker: Symbol::new(&e, "XLM"),
        };

        let admin2 = Address::generate(&e);
        let collateral_token = e.register_stellar_asset_contract_v2(admin2.clone());
        let collateral_asset = StellarAssetClient::new(&e, &collateral_token.address());
        let collateral_token_client = TokenClient::new(&e, &collateral_token.address());
        let collateral_currency = loan_pool::Currency {
            token_address: collateral_token.address(),
            ticker: Symbol::new(&e, "USDC"),
        };

        // Register mock Reflector contract.
        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        // Mint the user some coins
        let user = Address::generate(&e);
        collateral_asset.mint(&user, &1_000_000);

        assert_eq!(collateral_token_client.balance(&user), 1_000_000);

        // Set up a loan pool with funds for borrowing.
        let loan_pool_id = e.register(loan_pool::WASM, ());
        let loan_pool_client = loan_pool::Client::new(&e, &loan_pool_id);

        // Set up a loan_pool for the collaterals.
        let collateral_pool_id = e.register(loan_pool::WASM, ());
        let collateral_pool_client = loan_pool::Client::new(&e, &collateral_pool_id);

        // Register loan manager contract.
        let contract_id = e.register(LoanManager, ());
        let contract_client = LoanManagerClient::new(&e, &contract_id);

        // ACT
        // Initialize the loan pool and deposit some of the admin's funds.
        loan_pool_client.initialize(&contract_id, &loan_currency, &8_000_000);
        loan_pool_client.deposit(&admin, &10_001);

        collateral_pool_client.initialize(&contract_id, &collateral_currency, &8_000_000);

        // Create a loan.
        contract_client.create_loan(&user, &10_000, &loan_pool_id, &12_505, &collateral_pool_id);

        let user_loan = contract_client.get_loan(&user).unwrap();

        assert_eq!(user_loan.borrowed_amount, 10_000);

        contract_client.add_interest(&user);

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

        contract_client.add_interest(&user);

        let user_loan = contract_client.get_loan(&user).unwrap();

        assert_eq!(user_loan.borrowed_amount, 12_998);
        assert_eq!(user_loan.health_factor, 7_696_568);
        assert_eq!(user_loan.collateral_amount, 12_505);

        e.ledger().with_mut(|li| {
            li.sequence_number = 100_000 + 1_000;
        });

        let reflector_addr = Address::from_string(&String::from_str(&e, REFLECTOR_ADDRESS));
        e.register_at(&reflector_addr, oracle::WASM, ());

        contract_client.liquidate(&admin, &user, &5000);

        let user_loan = contract_client.get_loan(&user).unwrap();

        assert_eq!(user_loan.borrowed_amount, 7_998);
        assert_eq!(user_loan.health_factor, 7_256_814);
        assert_eq!(user_loan.collateral_amount, 7_255);
    }
}
