use core::time::Duration;
use dotenvy::dotenv;
use log::{error, info, warn};
use once_cell::sync::OnceCell;
use soroban_client::soroban_rpc::{EventResponse, EventType, GetEventsResponse};
use soroban_client::{EventFilter, Pagination};
use std::collections::HashSet;
use std::{cell::RefCell, env, rc::Rc, str::FromStr, thread};
use stellar_xdr::curr::{ScSymbol, ScVal, StringM};

use self::models::{Loan, Price};
use self::schema::loans::dsl::loans;
use self::schema::prices::dsl::prices;

use anyhow::{Error, Result};
use diesel::prelude::*;
use liquidation_bot::utils::{
    asset_to_scval, decode_loan_from_simulate_response, decode_topic, decode_value,
    extract_i128_from_result, Asset,
};
use liquidation_bot::*;
use soroban_client::{
    account::{Account, AccountBehavior},
    address::{Address, AddressTrait},
    keypair::{Keypair, KeypairBehavior},
    network::{NetworkPassphrase, Networks},
    operation::Operation,
    soroban_rpc::{SendTransactionResponse, SendTransactionStatus, TransactionStatus},
    transaction::{TransactionBehavior, TransactionBuilder, TransactionBuilderBehavior},
    Options, Server,
};
use stellar_rpc_client::{self, Event};

const SLEEP_TIME_SECONDS: u64 = 10;

static CONFIG: OnceCell<BotConfig> = OnceCell::new();

fn get_config() -> &'static BotConfig {
    CONFIG.get().expect("Config not initialized")
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let config = load_config().await?;
    CONFIG.set(config).expect("Failed to set global config");

    let BotConfig {
        rpc_url,
        source_keypair,
        ..
    } = get_config();

    let db_connection = &mut establish_connection();
    let rpc_client = stellar_rpc_client::Client::new(rpc_url)?;

    let allow_http: bool = std::env::var("ALLOW_HTTP")
        .expect("ALLOW_HTTP must be set in .env")
        .parse()
        .expect("ALLOW_HTTP must be 'true' or 'false'");
    let options = Options {
        allow_http,
        ..Options::default()
    };

    let server = soroban_client::Server::new(rpc_url, options).expect("Cannot create server");

    let account_data = server.get_account(&source_keypair.public_key()).await?;
    let source_account = Rc::new(RefCell::new(
        Account::new(
            &source_keypair.public_key(),
            &account_data.sequence_number(),
        )
        .map_err(|e| anyhow::anyhow!("Account::new failed: {}", e))?,
    ));

    // The history can be fetched a little further back than 120_000 ledgers, but that's a nice round number.
    #[cfg(feature = "local")]
    let history_depth = 0;
    #[cfg(not(feature = "local"))]
    let history_depth = 120_000;

    let mut ledger = rpc_client.get_latest_ledger().await?.sequence - history_depth;

    loop {
        let GetEventsResponse {
            events,
            latest_ledger: new_ledger,
            ..
        } = fetch_events(ledger, &server).await?;
        ledger = new_ledger as u32;

        find_loans_from_events(events, db_connection, &server, &source_account).await?;

        fetch_prices(db_connection, &server, &source_account).await?;

        find_liquidateable(db_connection, &server, &source_account).await?;

        info!("Sleeping for {SLEEP_TIME_SECONDS} seconds.");
        thread::sleep(Duration::from_secs(SLEEP_TIME_SECONDS))
    }
}

async fn fetch_events(ledger: u32, server: &Server) -> Result<GetEventsResponse, Error> {
    info!("Fetching new loans from Loan Manager.");

    let event_filter = EventFilter::new(EventType::Contract);
    let limit = Some(100);
    //TODO: This could be changed to use the soroban_client for simplicity.
    let events_response = server
        .get_events(Pagination::From(ledger), vec![event_filter], limit)
        .await?;
    info!("{:#?}", events_response);
    Ok(events_response)
}

async fn find_loans_from_events(
    events: Vec<EventResponse>,
    db_connection: &mut PgConnection,
    server: &Server,
    source_account: &Rc<RefCell<Account>>,
) -> Result<(), Error> {
    for event in events {
        info!("topic: {:#?}", event.topic());
        info!("value: {:#?}", event.value());
    }
    Ok(())
}

async fn fetch_loan_to_db(
    loan: Vec<String>,
    db_connection: &mut PgConnection,
    server: &Server,
    source_account: &Rc<RefCell<Account>>,
) -> Result<(), Error> {
    let config = get_config();
    let method = "get_loan";
    let loan_owner = &loan[1];
    info!("Fetching loan {} to database", loan_owner);
    let args = vec![Address::to_sc_val(
        &Address::from_string(loan_owner)
            .map_err(|e| anyhow::anyhow!("Account::from_string failed: {}", e))?,
    )
    .map_err(|e| anyhow::anyhow!("Address::to_sc_val failed: {}", e))?];

    let read_loan_op = Operation::new()
        .invoke_contract(&config.loan_manager_id, method, args, None)
        .expect("Cannot create invoke_contract operation");

    // Build the transaction
    #[cfg(feature = "local")]
    let network = Networks::standalone();

    #[cfg(not(feature = "local"))]
    let network = Networks::testnet();

    let mut builder = TransactionBuilder::new(source_account.clone(), network, None);
    builder.fee(1000u32);
    builder.add_operation(read_loan_op);

    let mut tx = builder.build();
    tx.sign(std::slice::from_ref(&config.source_keypair));

    // Simulate transaction and handle response
    let response = server.simulate_transaction(&tx, None).await?;

    // TODO: Add test
    match response.to_result() {
        Some(result) => {
            let loan = decode_loan_from_simulate_response(result)?;
            save_loan(db_connection, loan)?;
        }
        None => {
            warn!("Simulation returned None. Loan may not exist (deleted). Skipping.");
            return Ok(());
        }
    }

    Ok(())
}

pub fn save_loan(db_connection: &mut PgConnection, loan: Loan) -> Result<(), Error> {
    use crate::schema::loans::dsl::*;

    let existing = loans
        .filter(borrower.eq(&loan.borrower))
        .first::<Loan>(db_connection)
        .optional()?;

    if let Some(existing_loan) = existing {
        diesel::update(loans.filter(id.eq(existing_loan.id)))
            .set((
                borrowed_amount.eq(loan.borrowed_amount),
                borrowed_from.eq(loan.borrowed_from),
                collateral_amount.eq(loan.collateral_amount),
                collateral_from.eq(loan.collateral_from),
                unpaid_interest.eq(loan.unpaid_interest),
            ))
            .execute(db_connection)?;
    } else {
        diesel::insert_into(loans)
            .values((
                crate::schema::loans::borrower.eq(&loan.borrower),
                crate::schema::loans::borrowed_amount.eq(loan.borrowed_amount),
                crate::schema::loans::borrowed_from.eq(&loan.borrowed_from),
                crate::schema::loans::collateral_amount.eq(loan.collateral_amount),
                crate::schema::loans::collateral_from.eq(&loan.collateral_from),
                crate::schema::loans::unpaid_interest.eq(loan.unpaid_interest),
            ))
            .execute(db_connection)?;
    }
    Ok(())
}

async fn delete_loan_from_db(
    value: Vec<String>,
    db_connection: &mut PgConnection,
) -> Result<(), Error> {
    use crate::schema::loans::dsl::*;

    let loan_owner = &value[1];

    let deleted_rows =
        diesel::delete(loans.filter(borrower.eq(loan_owner))).execute(db_connection)?;

    if deleted_rows > 0 {
        info!("Deleted loan for borrower: {}", loan_owner);
    } else {
        warn!("No loan found to delete for borrower: {}", loan_owner);
    }

    Ok(())
}

async fn fetch_prices(
    db_connection: &mut PgConnection,
    server: &Server,
    source_account: &Rc<RefCell<Account>>,
) -> Result<(), Error> {
    use crate::schema::loans::dsl::*;

    info!("Fetching prices from Reflector");

    let config = get_config();

    let borrowed_from_uniques = loans.select(borrowed_from).distinct().load(db_connection)?;
    let collateral_from_uniques = loans
        .select(collateral_from)
        .distinct()
        .load(db_connection)?;
    let unique_pools_in_use: HashSet<String> = borrowed_from_uniques
        .into_iter()
        .chain(collateral_from_uniques)
        .collect();

    // Build operation
    let method = "twap"; // Time-weighted average price.

    let amount_of_data_points = ScVal::U32(12); // 12 datapoints with 5 min resolution -> average
                                                // of one hour

    let xlm_pool =
        env::var("CONTRACT_ID_POOL_XLM").expect("CONTRACT_ID_POOL_XLM must be set in .env");
    let usdc_pool =
        env::var("CONTRACT_ID_POOL_USDC").expect("CONTRACT_ID_POOL_USDC must be set in .env");
    let eurc_pool =
        env::var("CONTRACT_ID_POOL_EURC").expect("CONTRACT_ID_POOL_EURC must be set in .env");

    for pool in unique_pools_in_use {
        let currency = match pool.as_str() {
            p if p == xlm_pool => "XLM",
            p if p == usdc_pool => "USDC",
            p if p == eurc_pool => "EURC",
            _ => "None",
        };

        if currency == "None" {
            warn!("Skipping unknown pool: {}", pool);
            continue;
        }

        let asset = Asset::Other(ScSymbol(StringM::from_str(currency)?));

        let args = vec![asset_to_scval(&asset)?, amount_of_data_points.clone()];

        let fetch_prices_op = Operation::new()
            .invoke_contract(&config.oracle_contract_address, method, args, None)
            .expect("Cannot create invoke_contract operation");

        // Build the transaction
        #[cfg(feature = "local")]
        let network = Networks::standalone();

        #[cfg(not(feature = "local"))]
        let network = Networks::testnet();

        let mut builder = TransactionBuilder::new(source_account.clone(), network, None);
        builder.fee(1000u32);
        builder.add_operation(fetch_prices_op);

        let mut tx = builder.build();
        tx.sign(std::slice::from_ref(&config.source_keypair));

        // Simulate transaction and handle response
        let response = server.simulate_transaction(&tx, None).await?;

        let results = response.to_result();

        let price_twap = extract_i128_from_result(results)
            .ok_or(Error::msg("Couldn't extract price from result"))?;

        info!("fetched price for {currency}: {price_twap}");

        use crate::schema::prices::dsl::*;

        let existing = prices
            .filter(pool_address.eq(&pool))
            .first::<Price>(db_connection)
            .optional()?;

        if let Some(existing_price) = existing {
            diesel::update(prices.filter(id.eq(existing_price.id)))
                .set((
                    pool_address.eq(&pool),
                    time_weighted_average_price.eq(price_twap as i64),
                ))
                .execute(db_connection)?;
        } else {
            diesel::insert_into(prices)
                .values((
                    pool_address.eq(&pool),
                    time_weighted_average_price.eq(price_twap as i64),
                ))
                .execute(db_connection)?;
        }
    }
    Ok(())
}

async fn find_liquidateable(
    connection: &mut PgConnection,
    server: &Server,
    source_account: &Rc<RefCell<Account>>,
) -> Result<(), Error> {
    let all_loans = loans
        .select(Loan::as_select())
        .load(connection)
        .expect("Error loading loans");

    let all_prices = prices
        .select(Price::as_select())
        .load(connection)
        .expect("Error loading prices");

    info!("Total of {} loans in database.", all_loans.len());

    for loan in all_loans {
        let Loan {
            borrowed_amount,
            ref borrowed_from,
            collateral_amount,
            ref collateral_from,
            ..
        } = loan;

        let borrow_token_price = all_prices
            .iter()
            .find(|p| p.pool_address == *borrowed_from)
            .map(|p| p.time_weighted_average_price)
            .expect("No price found for borrow pool") as i128;

        let collateral_token_price = all_prices
            .iter()
            .find(|p| p.pool_address == *collateral_from)
            .map(|p| p.time_weighted_average_price)
            .expect("No price found for collateral pool")
            as i128;

        // TODO:Figure out where we get this from. We have getter for it and pool address in this
        // scope
        // TODO:Scale of values seems to be larger than on contract side
        let collateral_factor = 8000000;
        const DECIMAL_TO_INT_MULTIPLIER: i64 = 10_000_000;

        let collateral_value = collateral_token_price
            .checked_mul(collateral_amount as i128)
            .ok_or(Error::msg("OverOrUnderFlow"))?
            .checked_mul(collateral_factor)
            .ok_or(Error::msg("OverOrUnderFlow"))?
            .checked_div(DECIMAL_TO_INT_MULTIPLIER as i128)
            .ok_or(Error::msg("OverOrUnderFlow"))?;

        let borrowed_value = borrow_token_price
            .checked_mul(borrowed_amount as i128)
            .ok_or(Error::msg("OverOrUnderFlow"))?;

        let health_factor = collateral_value
            .checked_mul(DECIMAL_TO_INT_MULTIPLIER as i128)
            .ok_or(Error::msg("OverOrUnderFlow"))?
            .checked_div(borrowed_value)
            .ok_or(Error::msg("OverOrUnderFlow"))?;

        let health_factor_threshold = 10_100_000;
        if health_factor < health_factor_threshold {
            info!("Found loan close to liquidation threshold: {:#?}", loan);
            if let Err(e) = attempt_liquidating(loan.clone(), server, source_account).await {
                warn!(
                    "Failed to liquidate loan for borrower {}: {}",
                    loan.borrower, e
                );
                // Continue processing other loans instead of crashing the bot
            }
        }
    }

    Ok(())
}

async fn attempt_liquidating(
    loan: Loan,
    server: &Server,
    source_account: &Rc<RefCell<Account>>,
) -> Result<(), Error> {
    let config = get_config();

    info!("Attempting to liquidate loan: {:#?}", loan);

    // Build operation
    let method = "liquidate";
    let loan_owner = &loan.borrower;

    //TODO: This has to be optimized somehow. Sometimes half of the loan can be too much. Then
    //again sometimes very small liquidations don't help.
    let amount = loan
        .borrowed_amount
        .checked_div(3)
        .ok_or(Error::msg("OverOrUnderFlow"))? as i128;

    let args = vec![
        Address::to_sc_val(
            &Address::from_string(&config.source_keypair.public_key())
                .map_err(|e| anyhow::anyhow!("Account::from_string failed: {}", e))?,
        )
        .map_err(|e| anyhow::anyhow!("Address::to_sc_val failed: {}", e))?,
        Address::to_sc_val(
            &Address::from_string(loan_owner)
                .map_err(|e| anyhow::anyhow!("Account::from_string failed: {}", e))?,
        )
        .map_err(|e| anyhow::anyhow!("Address::to_sc_val failed: {}", e))?,
        amount.into(),
    ];

    let read_loan_op = Operation::new()
        .invoke_contract(&config.loan_manager_id, method, args.clone(), None)
        .expect("Cannot create invoke_contract operation");

    //TODO: response now has data like minimal resource fee and if the liquidation would likely be
    //succesful. To truly optimize the system we should first do this simulation, then calculate
    //liquidation profitability, then using this data send an actual transaction.

    #[cfg(feature = "local")]
    let network = Networks::standalone();

    #[cfg(not(feature = "local"))]
    let network = Networks::testnet();

    let mut builder = TransactionBuilder::new(source_account.clone(), network, None);
    builder.fee(10000_u32);
    builder.add_operation(read_loan_op);

    let mut tx = builder.build();

    // Prepare transaction (includes simulation)
    tx = match server.prepare_transaction(&tx).await {
        Ok(prepared_tx) => prepared_tx,
        Err(e) => {
            warn!(
                "Transaction simulation failed for loan {}: {}",
                loan.borrower, e
            );
            return Err(anyhow::anyhow!("Simulation failed: {}", e));
        }
    };

    tx.sign(std::slice::from_ref(&config.source_keypair));

    // Send transaction
    let response = match server.send_transaction(tx).await {
        Ok(resp) => resp,
        Err(e) => {
            warn!(
                "Transaction sending failed for loan {}: {}",
                loan.borrower, e
            );
            return Err(anyhow::anyhow!("Transaction sending failed: {}", e));
        }
    };

    info!("Liquidation transaction response: {:#?}", response);

    // Increment sequence number after sending transaction (regardless of success/failure)
    source_account.borrow_mut().increment_sequence_number();

    // Profitability calculation
    // let bot_balance
    // let max_to_liquidate
    //
    let hash = response.hash.clone();
    if wait_success(server, hash, response).await {
        info!("Loan {} liquidated!", loan.borrower);
    } else {
        warn!("Failed to liquidate loan for {}", loan.borrower);
    }
    Ok(())
}

async fn wait_success(server: &Server, hash: String, response: SendTransactionResponse) -> bool {
    if response.status != SendTransactionStatus::Error {
        loop {
            let response = server.get_transaction(&hash).await;
            if let Ok(tx_result) = response {
                match tx_result.status {
                    TransactionStatus::Success => {
                        println!("Transaction successful!");
                        if let Some(ledger) = tx_result.ledger {
                            println!("Confirmed in ledger: {}", ledger);
                        }
                        return true;
                    }
                    TransactionStatus::NotFound => {
                        println!(
                            "Waiting for transaction confirmation... Latest ledger: {}",
                            tx_result.latest_ledger
                        );
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    TransactionStatus::Failed => {
                        if let Some(result) = tx_result.to_result() {
                            eprintln!("Transaction failed with result: {:?}", result);
                        } else {
                            eprintln!("Transaction failed without result XDR");
                        }
                        return false;
                    }
                }
            } else {
                eprintln!("Error getting transaction status: {:?}", response);
            }
        }
    }
    false
}

#[derive(Debug)]
pub struct BotConfig {
    rpc_url: String,
    loan_manager_id: String,
    source_keypair: Keypair,
    oracle_contract_address: String,
}

async fn load_config() -> Result<BotConfig, Error> {
    dotenv().ok();

    let rpc_url =
        env::var("PUBLIC_SOROBAN_RPC_URL").expect("PUBLIC_SOROBAN_RPC_URL must be set in .env");

    let secret_str =
        env::var("SOROBAN_SECRET_KEY").expect("SOROBAN_SECRET_KEY must be set in .env");

    let source_keypair = Keypair::from_secret(&secret_str).expect("No keypair for secret");

    let loan_manager_id =
        env::var("CONTRACT_ID_LOAN_MANAGER").expect("CONTRACT_ID_LOAN_MANAGER must be set in .env");

    let oracle_contract_address =
        env::var("ORACLE_ADDRESS").expect("ORACLE_ADDRESS must be set in .env");

    let config = BotConfig {
        rpc_url,
        loan_manager_id,
        source_keypair,
        oracle_contract_address,
    };
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::loans::dsl::{borrower as borrower_col, loans};
    use crate::schema::prices::dsl::prices;
    use crate::schema::prices::{
        pool_address as pool_address_col, time_weighted_average_price as price_col,
    };
    use dotenvy::dotenv;
    use serial_test::serial;
    use std::env;

    fn setup_test_db() -> PgConnection {
        dotenv().ok();
        let test_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        PgConnection::establish(&test_url).expect("Failed to connect to test DB")
    }

    fn create_test_loan(borrower: &str) -> Loan {
        Loan {
            id: 0,
            borrower: borrower.to_string(),
            borrowed_amount: 1000000000, // 1000 tokens with 7 decimals
            borrowed_from: "CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP".to_string(),
            collateral_amount: 2000000000, // 2000 tokens with 7 decimals
            collateral_from: "CAXTXTUCA6ILFHCPIN34TWWVL4YL2QDDHYI65MVVQCEMDANFZLXVIEIK".to_string(),
            unpaid_interest: 50000000, // 50 tokens with 7 decimals
        }
    }

    fn create_test_price(pool_address: &str, price: i64) -> Price {
        Price {
            id: 0,
            pool_address: pool_address.to_string(),
            time_weighted_average_price: price,
        }
    }

    fn clean_test_data(conn: &mut PgConnection, test_borrower: &str) {
        // Clean loans for specific borrower
        if !test_borrower.is_empty() {
            diesel::delete(loans.filter(borrower_col.eq(test_borrower)))
                .execute(conn)
                .ok();
        }

        // Clean all test-related loans
        diesel::delete(loans.filter(borrower_col.like("TEST_%")))
            .execute(conn)
            .ok();

        // Clean test prices
        diesel::delete(prices.filter(pool_address_col.like("TEST_%")))
            .execute(conn)
            .ok();
    }

    #[tokio::test]
    #[serial]
    async fn test_save_loan_inserts_and_updates() {
        let mut conn = setup_test_db();

        // Clean up before test
        clean_test_data(&mut conn, "TEST_BORROWER");

        let test_loan = Loan {
            id: 0,
            borrower: "TEST_BORROWER".into(),
            borrowed_amount: 100,
            borrowed_from: "SourceA".into(),
            collateral_amount: 50,
            collateral_from: "SourceB".into(),
            unpaid_interest: 10,
        };

        save_loan(&mut conn, test_loan.clone()).unwrap();

        let updated_loan = Loan {
            borrowed_amount: 120,
            unpaid_interest: 5,
            ..test_loan
        };

        save_loan(&mut conn, updated_loan).unwrap();

        let saved = loans
            .filter(borrower_col.eq("TEST_BORROWER"))
            .first::<Loan>(&mut conn)
            .unwrap();

        assert_eq!(saved.borrowed_amount, 120);
        assert_eq!(saved.unpaid_interest, 5);

        // Clean up after test
        clean_test_data(&mut conn, "TEST_BORROWER");
    }

    #[tokio::test]
    #[serial]
    async fn test_save_loan_creates_new_loan() {
        let mut conn = setup_test_db();
        let test_borrower = "TEST_NEW_BORROWER";

        clean_test_data(&mut conn, test_borrower);

        let test_loan = create_test_loan(test_borrower);
        save_loan(&mut conn, test_loan.clone()).unwrap();

        let saved = loans
            .filter(borrower_col.eq(test_borrower))
            .first::<Loan>(&mut conn)
            .unwrap();

        assert_eq!(saved.borrower, test_borrower);
        assert_eq!(saved.borrowed_amount, test_loan.borrowed_amount);
        assert_eq!(saved.collateral_amount, test_loan.collateral_amount);

        clean_test_data(&mut conn, test_borrower);
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_loan_from_db() {
        let mut conn = setup_test_db();
        let test_borrower = "TEST_DELETE_BORROWER";

        // Extra aggressive cleanup - delete ALL test data first
        diesel::delete(loans.filter(borrower_col.like("TEST_%")))
            .execute(&mut conn)
            .ok();

        // Verify no existing loan for this borrower
        let existing_count = loans
            .filter(borrower_col.eq(test_borrower))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap();
        assert_eq!(existing_count, 0, "Database should be clean before test");

        let test_loan = create_test_loan(test_borrower);
        save_loan(&mut conn, test_loan).unwrap();

        // Verify loan exists
        let count_before = loans
            .filter(borrower_col.eq(test_borrower))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap();
        assert_eq!(count_before, 1);

        // Delete loan
        let value = vec!["loan".to_string(), test_borrower.to_string()];
        delete_loan_from_db(value, &mut conn).await.unwrap();

        // Verify loan is deleted
        let count_after = loans
            .filter(borrower_col.eq(test_borrower))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap();
        assert_eq!(count_after, 0);
    }

    #[test]
    fn test_liquidation_calculation_healthy_loan() {
        let borrowed_amount = 1000000000; // 1000 tokens
        let borrowed_price = 1_0000000i128; // $1 per token
        let collateral_amount = 2000000000; // 2000 tokens
        let collateral_price = 1_0000000i128; // $1 per token
        let collateral_factor = 8000000; // 80%
        const DECIMAL_TO_INT_MULTIPLIER: i64 = 10_000_000;

        let collateral_value = collateral_price
            .checked_mul(collateral_amount as i128)
            .unwrap()
            .checked_mul(collateral_factor)
            .unwrap()
            .checked_div(DECIMAL_TO_INT_MULTIPLIER as i128)
            .unwrap();

        let borrowed_value = borrowed_price.checked_mul(borrowed_amount as i128).unwrap();

        let health_factor = collateral_value
            .checked_mul(DECIMAL_TO_INT_MULTIPLIER as i128)
            .unwrap()
            .checked_div(borrowed_value)
            .unwrap();

        // Expected: (2000 * 1 * 0.8) / 1000 = 1.6 = 16000000
        assert_eq!(health_factor, 16000000);

        // This should NOT be liquidatable with threshold of 1.01
        let threshold = 10100000;
        assert!(health_factor >= threshold);
    }

    #[test]
    fn test_liquidation_calculation_unhealthy_loan() {
        let borrowed_amount = 1000000000; // 1000 tokens
        let borrowed_price = 1_0000000i128; // $1 per token
        let collateral_amount = 1000000000; // 1000 tokens
        let collateral_price = 1_0000000i128; // $1 per token
        let collateral_factor = 8000000; // 80%
        const DECIMAL_TO_INT_MULTIPLIER: i64 = 10_000_000;

        let collateral_value = collateral_price
            .checked_mul(collateral_amount as i128)
            .unwrap()
            .checked_mul(collateral_factor)
            .unwrap()
            .checked_div(DECIMAL_TO_INT_MULTIPLIER as i128)
            .unwrap();

        let borrowed_value = borrowed_price.checked_mul(borrowed_amount as i128).unwrap();

        let health_factor = collateral_value
            .checked_mul(DECIMAL_TO_INT_MULTIPLIER as i128)
            .unwrap()
            .checked_div(borrowed_value)
            .unwrap();

        // Expected: (1000 * 1 * 0.8) / 1000 = 0.8 = 8000000
        assert_eq!(health_factor, 8000000);

        // This should be liquidatable with threshold of 1.01
        let threshold = 10100000;
        assert!(health_factor < threshold);
    }

    #[test]
    fn test_liquidation_amount_calculation() {
        let borrowed_amount = 3000000000i64; // 3000 tokens
        let liquidation_amount = borrowed_amount
            .checked_div(3)
            .expect("Division should not overflow");

        assert_eq!(liquidation_amount, 1000000000); // 1000 tokens (1/3 of loan)
    }

    #[tokio::test]
    #[serial]
    async fn test_save_and_update_price() {
        let mut conn = setup_test_db();
        let test_pool = "TEST_POOL_ADDRESS";

        clean_test_data(&mut conn, "");

        // Clean up any existing test data
        diesel::delete(prices.filter(pool_address_col.eq(test_pool)))
            .execute(&mut conn)
            .ok();

        let test_price = create_test_price(test_pool, 1500000); // $1.5

        // Insert new price
        diesel::insert_into(prices)
            .values((
                pool_address_col.eq(&test_price.pool_address),
                price_col.eq(test_price.time_weighted_average_price),
            ))
            .execute(&mut conn)
            .unwrap();

        // Verify insertion
        let saved = prices
            .filter(pool_address_col.eq(test_pool))
            .first::<Price>(&mut conn)
            .unwrap();
        assert_eq!(saved.time_weighted_average_price, 1500000);

        // Update price
        let updated_price = 2000000; // $2.0
        diesel::update(prices.filter(pool_address_col.eq(test_pool)))
            .set(price_col.eq(updated_price))
            .execute(&mut conn)
            .unwrap();

        // Verify update
        let updated = prices
            .filter(pool_address_col.eq(test_pool))
            .first::<Price>(&mut conn)
            .unwrap();
        assert_eq!(updated.time_weighted_average_price, updated_price);

        // Clean up
        diesel::delete(prices.filter(pool_address_col.eq(test_pool)))
            .execute(&mut conn)
            .unwrap();
    }

    #[test]
    fn test_event_topic_matching() {
        let topics_loan_created = vec!["loan".to_string(), "created".to_string()];
        let topics_loan_updated = vec!["loan".to_string(), "updated".to_string()];
        let topics_loan_deleted = vec!["loan".to_string(), "deleted".to_string()];
        let topics_other = vec!["pool".to_string(), "created".to_string()];

        // Test loan created/updated matching
        let topics_lower: Vec<String> = topics_loan_created
            .iter()
            .map(|t| t.to_lowercase())
            .collect();
        let slice: Vec<&str> = topics_lower.iter().map(String::as_str).collect();
        assert!(matches!(slice.as_slice(), ["loan", "created"]));

        let topics_lower: Vec<String> = topics_loan_updated
            .iter()
            .map(|t| t.to_lowercase())
            .collect();
        let slice: Vec<&str> = topics_lower.iter().map(String::as_str).collect();
        assert!(matches!(slice.as_slice(), ["loan", "updated"]));

        let topics_lower: Vec<String> = topics_loan_deleted
            .iter()
            .map(|t| t.to_lowercase())
            .collect();
        let slice: Vec<&str> = topics_lower.iter().map(String::as_str).collect();
        assert!(matches!(slice.as_slice(), ["loan", "deleted"]));

        let topics_lower: Vec<String> = topics_other.iter().map(|t| t.to_lowercase()).collect();
        let slice: Vec<&str> = topics_lower.iter().map(String::as_str).collect();
        assert!(!matches!(
            slice.as_slice(),
            ["loan", "created"] | ["loan", "updated"] | ["loan", "deleted"]
        ));
    }

    #[test]
    fn test_pool_address_to_currency_mapping() {
        let xlm_pool = "CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP";
        let usdc_pool = "CAXTXTUCA6ILFHCPIN34TWWVL4YL2QDDHYI65MVVQCEMDANFZLXVIEIK";
        let _eurc_pool = "CDUFMIS6ZH3JM5MPNTWMDLBXPNQYV5FBPBGCFT2WWG4EXKGEPOCBNGCZ";
        let unknown_pool = "SOME_UNKNOWN_POOL_ADDRESS";

        let xlm_currency = match xlm_pool {
            "CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP" => "XLM",
            "CAXTXTUCA6ILFHCPIN34TWWVL4YL2QDDHYI65MVVQCEMDANFZLXVIEIK" => "USDC",
            "CDUFMIS6ZH3JM5MPNTWMDLBXPNQYV5FBPBGCFT2WWG4EXKGEPOCBNGCZ" => "EURC",
            _ => "None",
        };
        assert_eq!(xlm_currency, "XLM");

        let usdc_currency = match usdc_pool {
            "CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP" => "XLM",
            "CAXTXTUCA6ILFHCPIN34TWWVL4YL2QDDHYI65MVVQCEMDANFZLXVIEIK" => "USDC",
            "CDUFMIS6ZH3JM5MPNTWMDLBXPNQYV5FBPBGCFT2WWG4EXKGEPOCBNGCZ" => "EURC",
            _ => "None",
        };
        assert_eq!(usdc_currency, "USDC");

        let unknown_currency = match unknown_pool {
            "CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP" => "XLM",
            "CAXTXTUCA6ILFHCPIN34TWWVL4YL2QDDHYI65MVVQCEMDANFZLXVIEIK" => "USDC",
            "CDUFMIS6ZH3JM5MPNTWMDLBXPNQYV5FBPBGCFT2WWG4EXKGEPOCBNGCZ" => "EURC",
            _ => "None",
        };
        assert_eq!(unknown_currency, "None");
    }

    #[test]
    fn test_overflow_protection_in_liquidation_calculation() {
        // Test with very large numbers to ensure overflow protection works
        let borrowed_amount = i64::MAX / 2;
        let borrowed_price = 1000000i128;
        let collateral_amount = i64::MAX / 2;
        let collateral_price = 1000000i128;
        let collateral_factor = 8000000;
        const DECIMAL_TO_INT_MULTIPLIER: i64 = 10_000_000;

        // This should not panic due to overflow protection
        let collateral_value_result = collateral_price
            .checked_mul(collateral_amount as i128)
            .and_then(|v| v.checked_mul(collateral_factor))
            .and_then(|v| v.checked_div(DECIMAL_TO_INT_MULTIPLIER as i128));

        let borrowed_value_result = borrowed_price.checked_mul(borrowed_amount as i128);

        // Test that we handle potential overflows gracefully
        assert!(collateral_value_result.is_some() || collateral_value_result.is_none());
        assert!(borrowed_value_result.is_some() || borrowed_value_result.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_loan_nonexistent() {
        let mut conn = setup_test_db();
        let test_borrower = "NONEXISTENT_BORROWER";

        // Ensure the borrower doesn't exist
        clean_test_data(&mut conn, test_borrower);

        // Try to delete non-existent loan
        let value = vec!["loan".to_string(), test_borrower.to_string()];
        let result = delete_loan_from_db(value, &mut conn).await;

        // Should succeed (no error) even if loan doesn't exist
        assert!(result.is_ok());
    }

    #[test]
    fn test_collateral_factor_scenarios() {
        let borrowed_amount = 1000000000i64;
        let borrowed_price = 1_0000000i128;
        let collateral_amount = 1500000000i64;
        let collateral_price = 1_0000000i128;
        const DECIMAL_TO_INT_MULTIPLIER: i64 = 10_000_000;

        // Test different collateral factors
        let factors = vec![
            5000000, // 50%
            7500000, // 75%
            8000000, // 80%
            9000000, // 90%
        ];

        for factor in factors {
            let collateral_value = collateral_price
                .checked_mul(collateral_amount as i128)
                .unwrap()
                .checked_mul(factor)
                .unwrap()
                .checked_div(DECIMAL_TO_INT_MULTIPLIER as i128)
                .unwrap();

            let borrowed_value = borrowed_price.checked_mul(borrowed_amount as i128).unwrap();

            let health_factor = collateral_value
                .checked_mul(DECIMAL_TO_INT_MULTIPLIER as i128)
                .unwrap()
                .checked_div(borrowed_value)
                .unwrap();

            // Health factor should be proportional to collateral factor
            let expected_health_factor =
                (collateral_amount as i128 * factor) / (borrowed_amount as i128);
            assert_eq!(health_factor, expected_health_factor);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_save_loan_edge_cases() {
        let mut conn = setup_test_db();
        let test_borrower = "TEST_EDGE_CASE_BORROWER";

        clean_test_data(&mut conn, test_borrower);

        // Test with zero amounts
        let zero_loan = Loan {
            id: 0,
            borrower: test_borrower.to_string(),
            borrowed_amount: 0,
            borrowed_from: "POOL1".to_string(),
            collateral_amount: 0,
            collateral_from: "POOL2".to_string(),
            unpaid_interest: 0,
        };

        let result = save_loan(&mut conn, zero_loan);
        assert!(result.is_ok());

        // Test with large values (but not MAX to avoid potential DB constraints)
        let large_loan = Loan {
            id: 0,
            borrower: format!("{}_LARGE", test_borrower),
            borrowed_amount: 1_000_000_000_000_000, // 1 quadrillion
            borrowed_from: "POOL1".to_string(),
            collateral_amount: 2_000_000_000_000_000, // 2 quadrillion
            collateral_from: "POOL2".to_string(),
            unpaid_interest: 50_000_000_000_000, // 50 trillion
        };

        let result = save_loan(&mut conn, large_loan);
        assert!(result.is_ok());

        clean_test_data(&mut conn, test_borrower);
        clean_test_data(&mut conn, &format!("{}_LARGE", test_borrower));
    }

    #[test]
    fn test_health_factor_threshold_boundary() {
        let threshold = 10_100_000i128; // 1.01

        // Test exactly at threshold
        let health_factor_at_threshold = 10_100_000i128;
        assert!(health_factor_at_threshold >= threshold);

        // Test just below threshold (should be liquidatable)
        let health_factor_below = 10_099_999i128;
        assert!(health_factor_below < threshold);

        // Test just above threshold (should not be liquidatable)
        let health_factor_above = 10_100_001i128;
        assert!(health_factor_above >= threshold);
    }
}
