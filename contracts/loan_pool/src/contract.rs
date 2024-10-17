use crate::pool::Currency;
use crate::positions;
use crate::{pool, storage_types::Positions};

use soroban_sdk::{contract, contractimpl, contractmeta, token, Address, BytesN, Env};

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
    }

    pub fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let loan_manager_addr = pool::read_loan_manager_addr(&e);
        loan_manager_addr.require_auth();

        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    /// Deposits token. Also, mints pool shares for the "user" Identifier.
    pub fn deposit(e: Env, user: Address, amount: i128) -> i128 {
        user.require_auth();
        assert!(amount > 0, "Amount must be positive!");

        let client = token::Client::new(&e, &pool::read_currency(&e).token_address);
        client.transfer(&user, &e.current_contract_address(), &amount);

        pool::change_available_balance(&e, amount);
        pool::change_total_shares(&e, amount);
        pool::change_total_balance(&e, amount);

        // Increase users position in pool as they deposit
        // as this is deposit amount is added to receivables and
        // liabilities & collateral stays intact
        let liabilities: i128 = 0; // temp test param
        let collateral: i128 = 0; // temp test param
        positions::increase_positions(&e, user.clone(), amount, liabilities, collateral);

        amount
    }

    /// Transfers share tokens back, burns them and gives corresponding amount of tokens back to user. Returns amount of tokens withdrawn
    pub fn withdraw(e: Env, user: Address, amount: i128) -> (i128, i128) {
        user.require_auth();

        // Get users receivables
        let Positions { receivables, .. } = positions::read_positions(&e, &user);

        // Check that user is not trying to move more than receivables (TODO: also include collateral?)
        assert!(
            amount <= receivables,
            "Amount can not be greater than receivables!"
        );

        // TODO: Decrease AvailableBalance
        pool::change_available_balance(&e, -amount);
        // TODO: Decrease TotalShares - Positions should have shares if we use them
        // TODO: Decrease TotalBalance
        pool::change_total_balance(&e, -amount);

        // Decrease users position in pool as they withdraw
        let liabilities: i128 = 0;
        let collateral: i128 = 0;
        positions::decrease_positions(&e, user.clone(), amount, liabilities, collateral);

        // Transfer tokens from pool to user
        let token_address = &pool::read_currency(&e).token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&e.current_contract_address(), &user, &amount);

        (amount, amount)
    }

    /// Borrow tokens from the pool
    pub fn borrow(e: Env, user: Address, amount: i128) -> i128 {
        /*
        Borrow should only be callable from the loans contract. This is as the loans contract will
        include the logic and checks that the borrowing can be actually done. Therefore we need to
        include a check that the caller is the loans contract.
        */
        let loan_manager_addr = pool::read_loan_manager_addr(&e);
        loan_manager_addr.require_auth();
        user.require_auth();

        let balance = pool::read_available_balance(&e);
        assert!(
            amount < balance,
            "Borrowed amount has to be less than available balance!"
        ); // Check that there is enough available balance

        pool::change_available_balance(&e, -amount);

        // Increase users position in pool as they deposit
        // as this is debt amount is added to liabilities and
        // collateral & receivables stays intact
        let collateral: i128 = 0; // temp test param
        let receivables: i128 = 0; // temp test param
        positions::increase_positions(&e, user.clone(), receivables, amount, collateral);

        let token_address = &pool::read_currency(&e).token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&e.current_contract_address(), &user, &amount);

        amount
    }

    /// Deposit tokens to the pool to be used as collateral
    pub fn deposit_collateral(e: Env, user: Address, amount: i128) -> i128 {
        user.require_auth();
        assert!(amount > 0, "Amount must be positive!");

        let token_address = &pool::read_currency(&e).token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&user, &e.current_contract_address(), &amount);

        // Increase users position in pool as they deposit
        // as this is collateral amount is added to collateral and
        // liabilities & receivables stays intact
        let liabilities: i128 = 0; // temp test param
        let receivables: i128 = 0; // temp test param
        positions::increase_positions(&e, user.clone(), receivables, liabilities, amount);

        amount
    }

    pub fn withdraw_collateral(e: Env, user: Address, amount: i128) -> i128 {
        user.require_auth();
        let loan_manager_addr = pool::read_loan_manager_addr(&e);
        loan_manager_addr.require_auth();
        assert!(amount > 0, "Amount must be positive!");

        let token_address = &pool::read_currency(&e).token_address;
        let client = token::Client::new(&e, token_address);
        client.transfer(&e.current_contract_address(), &user, &amount);

        // Increase users position in pool as they deposit
        // as this is collateral amount is added to collateral and
        // liabilities & receivables stays intact
        let liabilities: i128 = 0; // temp test param
        let receivables: i128 = 0; // temp test param
        positions::decrease_positions(&e, user.clone(), receivables, liabilities, amount);

        amount
    }

    /// Get user's positions in the pool
    pub fn get_user_positions(e: Env, user: Address) -> Positions {
        positions::read_positions(&e, &user)
    }

    /// Get contract data entries
    pub fn get_contract_balance(e: Env) -> i128 {
        pool::read_total_balance(&e)
    }

    pub fn get_available_balance(e: Env) -> i128 {
        pool::read_available_balance(&e)
    }

    pub fn get_currency(e: Env) -> Currency {
        pool::read_currency(&e)
    }

    pub fn increase_liabilities(e: Env, user: Address, amount: i128) {
        let loan_manager_addr = pool::read_loan_manager_addr(&e);
        loan_manager_addr.require_auth();

        positions::increase_positions(&e, user, 0, amount, 0);
    }

    pub fn repay(e: Env, user: Address, amount: i128, unpaid_interest: i128) {
        let loan_manager_addr = pool::read_loan_manager_addr(&e);
        loan_manager_addr.require_auth();

        let amount_to_admin = if amount < unpaid_interest {
            amount / 10
        } else {
            unpaid_interest / 10
        };

        let amount_to_pool = amount - amount_to_admin;

        let client = token::Client::new(&e, &pool::read_currency(&e).token_address);
        client.transfer(&user, &e.current_contract_address(), &amount_to_pool);
        client.transfer(&user, &loan_manager_addr, &amount_to_admin);

        positions::decrease_positions(&e, user, 0, amount, 0);
        pool::change_available_balance(&e, amount);
        pool::change_total_balance(&e, amount);
    }
}

#[cfg(test)]
mod test {
    use super::*; // This imports LoanPoolContract and everything else from the parent module
    use soroban_sdk::{
        testutils::Address as _,
        token::{Client as TokenClient, StellarAssetClient},
        Env, Symbol,
    };

    const TEST_LIQUIDATION_THRESHOLD: i128 = 800_000;

    #[test]
    fn initialize() {
        let e: Env = Env::default();
        e.mock_all_auths();

        let admin: Address = Address::generate(&e);
        let token_contract_id = e.register_stellar_asset_contract(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token_contract_id);
        let token = TokenClient::new(&e, &token_contract_id);
        let currency = Currency {
            token_address: token_contract_id,
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);
        assert_eq!(token.balance(&user), 1000);

        let contract_id = e.register_contract(None, LoanPoolContract);
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
        );
    }

    #[test]
    fn deposit() {
        let e: Env = Env::default();
        e.mock_all_auths();

        let admin: Address = Address::generate(&e);
        let token_contract_id = e.register_stellar_asset_contract(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token_contract_id);
        let token = TokenClient::new(&e, &token_contract_id);
        let currency = Currency {
            token_address: token_contract_id,
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);
        assert_eq!(token.balance(&user), 1000);

        let contract_id = e.register_contract(None, LoanPoolContract);
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

        let token_contract_id = e.register_stellar_asset_contract(admin.clone());
        let asset = StellarAssetClient::new(&e, &token_contract_id);

        let token_client = TokenClient::new(&e, &token_contract_id);
        let currency = Currency {
            token_address: token_contract_id,
            ticker: Symbol::new(&e, "XLM"),
        };

        let contract_id = e.register_contract(None, LoanPoolContract);
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
        let e: Env = Env::default();
        e.mock_all_auths();

        let admin: Address = Address::generate(&e);
        let token_contract_id = e.register_stellar_asset_contract(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token_contract_id);
        let token = TokenClient::new(&e, &token_contract_id);
        let currency = Currency {
            token_address: token_contract_id,
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);
        assert_eq!(token.balance(&user), 1000);

        let contract_id = e.register_contract(None, LoanPoolContract);
        let contract_client = LoanPoolContractClient::new(&e, &contract_id);
        let amount: i128 = 100;

        contract_client.initialize(
            &Address::generate(&e),
            &currency,
            &TEST_LIQUIDATION_THRESHOLD,
        );

        let result: i128 = contract_client.deposit(&user, &amount);

        assert_eq!(result, amount);

        let withdraw_result: (i128, i128) = contract_client.withdraw(&user, &amount);

        assert_eq!(withdraw_result, (amount, amount));
    }

    #[test]
    #[should_panic]
    fn deposit_more_than_account_balance() {
        let e: Env = Env::default();
        e.mock_all_auths();

        let admin: Address = Address::generate(&e);
        let token_contract_id = e.register_stellar_asset_contract(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token_contract_id);
        let token = TokenClient::new(&e, &token_contract_id);
        let currency = Currency {
            token_address: token_contract_id,
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);
        assert_eq!(token.balance(&user), 1000);

        let contract_id = e.register_contract(None, LoanPoolContract);
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
    #[should_panic(expected = "Amount can not be greater than receivables!")]
    fn withdraw_more_than_balance() {
        let e: Env = Env::default();
        e.mock_all_auths();

        let admin: Address = Address::generate(&e);
        let token_contract_id = e.register_stellar_asset_contract(admin.clone());
        let stellar_asset = StellarAssetClient::new(&e, &token_contract_id);
        let token = TokenClient::new(&e, &token_contract_id);
        let currency = Currency {
            token_address: token_contract_id,
            ticker: Symbol::new(&e, "XLM"),
        };

        let user = Address::generate(&e);
        stellar_asset.mint(&user, &1000);
        assert_eq!(token.balance(&user), 1000);

        let contract_id = e.register_contract(None, LoanPoolContract);
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
}
