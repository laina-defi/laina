use core::time::Duration;
use dotenvy::dotenv;
use log::{error, info, warn};
use soroban_sdk::xdr::{ScSymbol, StringM};
use std::collections::HashSet;
use std::{cell::RefCell, env, rc::Rc, str::FromStr, thread};
use stellar_xdr::curr::ScVal;

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
use stellar_rpc_client::{self, Event, EventStart, EventType, GetEventsResponse};

const SLEEP_TIME_SECONDS: u64 = 10;

pub struct BotConfig {
    loan_manager_id: String,
    server: Server,
    source_keypair: Keypair,
    source_public: String,
    source_account: Rc<RefCell<Account>>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let connection = &mut establish_connection();
    env_logger::init();

    // TODO:Decide on how to handle the initial ledger
    let mut ledger = 10;

    loop {
        let GetEventsResponse {
            events,
            latest_ledger: new_ledger,
        } = fetch_events(ledger).await?;
        ledger = new_ledger;
        find_loans_from_events(events, connection).await?;
        fetch_prices(connection).await?;
        find_liquidateable(connection).await?;

        info!("Sleeping for {SLEEP_TIME_SECONDS} seconds.");
        thread::sleep(Duration::from_secs(SLEEP_TIME_SECONDS))
    }
}

async fn fetch_events(ledger: u32) -> Result<GetEventsResponse, Error> {
    let BotConfig {
        loan_manager_id, ..
    } = load_config().await?;

    info!("Fetching new loans from Loan Manager.");
    let url =
        env::var("PUBLIC_SOROBAN_RPC_URL").expect("PUBLIC_SOROBAN_RPC_URL must be set in .env");
    let client = stellar_rpc_client::Client::new(&url)?;

    let start = EventStart::Ledger(ledger);
    let event_type = Some(EventType::Contract);
    let contract_ids = vec![loan_manager_id];
    let topics_slice = &[];
    let limit = Some(100);
    //TODO: This could be changed to use the soroban_client for simplicity.
    let events_response = client
        .get_events(start, event_type, &contract_ids, topics_slice, limit)
        .await?;
    Ok(events_response)
}

async fn find_loans_from_events(
    events: Vec<Event>,
    connection: &mut PgConnection,
) -> Result<(), Error> {
    for event in events {
        match decode_topic(event.topic.clone()) {
            Ok(topics) => {
                let topics_lower: Vec<String> = topics.iter().map(|t| t.to_lowercase()).collect();

                match topics_lower
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .as_slice()
                {
                    ["loan", "created"] | ["loan", "updated"] => {
                        match decode_value(event.value.clone()) {
                            Ok(value) => {
                                info!("Loan modified");
                                fetch_loan_to_db(value, connection).await?;
                            }
                            Err(e) => error!("Failed to decode value: {e}"),
                        }
                    }
                    ["loan", "deleted"] => match decode_value(event.value.clone()) {
                        Ok(value) => {
                            info!("Loan removed");
                            delete_loan_from_db(value, connection).await?;
                        }
                        Err(e) => error!("Failed to decode value: {e}"),
                    },
                    _ => {
                        warn!("Not a loan event, skipping.");
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to decode topic: {e}");
                continue;
            }
        }
    }
    Ok(())
}

async fn fetch_loan_to_db(loan: Vec<String>, connection: &mut PgConnection) -> Result<(), Error> {
    let BotConfig {
        server,
        source_keypair,
        loan_manager_id,
        source_account,
        ..
    } = load_config().await?;

    let method = "get_loan";
    let loan_owner = &loan[1];
    info!("Fetching loan {} to database", loan_owner);
    let args = vec![Address::to_sc_val(
        &Address::from_string(loan_owner)
            .map_err(|e| anyhow::anyhow!("Account::from_string failed: {}", e))?,
    )
    .map_err(|e| anyhow::anyhow!("Address::to_sc_val failed: {}", e))?];

    let read_loan_op = Operation::new()
        .invoke_contract(&loan_manager_id, method, args, None)
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
    tx.sign(&[source_keypair.clone()]);

    // Simulate transaction and handle response
    let response = server.simulate_transaction(tx, None).await?;

    // TODO: Add test
    match response.to_result() {
        Some(result) => {
            let loan = decode_loan_from_simulate_response(result)?;
            save_loan(connection, loan)?;
        }
        None => {
            warn!("Simulation returned None. Loan may not exist (deleted). Skipping.");
            return Ok(());
        }
    }

    Ok(())
}

pub fn save_loan(connection: &mut PgConnection, loan: Loan) -> Result<(), Error> {
    use crate::schema::loans::dsl::*;

    let existing = loans
        .filter(borrower.eq(&loan.borrower))
        .first::<Loan>(connection)
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
            .execute(connection)?;
    } else {
        diesel::insert_into(loans)
            .values(&loan)
            .execute(connection)?;
    }
    Ok(())
}

async fn delete_loan_from_db(
    value: Vec<String>,
    connection: &mut PgConnection,
) -> Result<(), Error> {
    use crate::schema::loans::dsl::*;

    let loan_owner = &value[1];

    let deleted_rows = diesel::delete(loans.filter(borrower.eq(loan_owner))).execute(connection)?;

    if deleted_rows > 0 {
        info!("Deleted loan for borrower: {}", loan_owner);
    } else {
        warn!("No loan found to delete for borrower: {}", loan_owner);
    }

    Ok(())
}

async fn fetch_prices(connection: &mut PgConnection) -> Result<(), Error> {
    info!("Fetching prices from Reflector");
    use crate::schema::loans::dsl::*;

    let BotConfig {
        server,
        source_keypair,
        source_account,
        ..
    } = load_config().await?;

    let borrowed_from_uniques = loans.select(borrowed_from).distinct().load(connection)?;
    let collateral_from_uniques = loans.select(collateral_from).distinct().load(connection)?;
    let unique_pools_in_use: HashSet<String> = borrowed_from_uniques
        .into_iter()
        .chain(collateral_from_uniques)
        .collect();

    // Build operation
    let oracle_contract_address =
        env::var("ORACLE_ADDRESS").expect("ORACLE_ADDRESS must be set in .env");
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
            .invoke_contract(&oracle_contract_address, method, args, None)
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
        tx.sign(&[source_keypair.clone()]);

        // Simulate transaction and handle response
        let response = server.simulate_transaction(tx, None).await?;

        let results = response.to_result();

        let price_twap = extract_i128_from_result(results)
            .ok_or(Error::msg("Couldn't extract price from result"))?;

        info!("fetched price for {currency}: {price_twap}");

        use crate::schema::prices::dsl::*;

        let existing = prices
            .filter(pool_address.eq(&pool))
            .first::<Price>(connection)
            .optional()?;

        if let Some(existing_price) = existing {
            diesel::update(prices.filter(id.eq(existing_price.id)))
                .set((
                    pool_address.eq(&pool),
                    time_weighted_average_price.eq(price_twap as i64),
                ))
                .execute(connection)?;
        } else {
            diesel::insert_into(prices)
                .values((
                    pool_address.eq(&pool),
                    time_weighted_average_price.eq(price_twap as i64),
                ))
                .execute(connection)?;
        }
    }
    Ok(())
}

async fn find_liquidateable(connection: &mut PgConnection) -> Result<(), Error> {
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
            if let Err(e) = attempt_liquidating(loan.clone()).await {
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

async fn attempt_liquidating(loan: Loan) -> Result<(), Error> {
    info!("Attempting to liquidate loan: {:#?}", loan);

    let BotConfig {
        source_keypair,
        server,
        loan_manager_id,
        source_public,
        ..
    } = load_config().await?;

    // Refresh account data before transaction to ensure correct sequence number
    let account_data = server.get_account(&source_public).await?;
    let source_account = Rc::new(RefCell::new(
        Account::new(&source_public, &account_data.sequence_number())
            .map_err(|e| anyhow::anyhow!("Account::new failed: {}", e))?,
    ));

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
            &Address::from_string(&source_public)
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
        .invoke_contract(&loan_manager_id, method, args.clone(), None)
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
    tx = match server.prepare_transaction(tx).await {
        Ok(prepared_tx) => prepared_tx,
        Err(e) => {
            warn!(
                "Transaction simulation failed for loan {}: {}",
                loan.borrower, e
            );
            return Err(anyhow::anyhow!("Simulation failed: {}", e));
        }
    };

    tx.sign(&[source_keypair.clone()]);

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
    if wait_success(&server, hash, response).await {
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

async fn load_config() -> Result<BotConfig, Error> {
    dotenv().ok();

    let url =
        env::var("PUBLIC_SOROBAN_RPC_URL").expect("PUBLIC_SOROBAN_RPC_URL must be set in .env");

    let allow_http: bool = std::env::var("ALLOW_HTTP")
        .expect("ALLOW_HTTP must be set in .env")
        .parse()
        .expect("ALLOW_HTTP must be 'true' or 'false'");
    let options = Options {
        allow_http: allow_http,
        ..Options::default()
    };

    let server = soroban_client::Server::new(&url, options).expect("Cannot create server");

    let secret_str =
        env::var("SOROBAN_SECRET_KEY").expect("SOROBAN_SECRET_KEY must be set in .env");

    let source_keypair = Keypair::from_secret(&secret_str).expect("No keypair for secret");
    let loan_manager_id =
        env::var("CONTRACT_ID_LOAN_MANAGER").expect("CONTRACT_ID_LOAN_MANAGER must be set in .env");

    let source_public = source_keypair.public_key();

    let account_data = server.get_account(&source_public).await?;
    let source_account = Rc::new(RefCell::new(
        Account::new(&source_public, &account_data.sequence_number())
            .map_err(|e| anyhow::anyhow!("Account::new failed: {}", e))?,
    ));

    let config = BotConfig {
        loan_manager_id,
        server,
        source_keypair,
        source_public,
        source_account,
    };
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenvy::dotenv;
    use std::env;

    fn setup_test_db() -> PgConnection {
        dotenv().ok();
        let test_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        PgConnection::establish(&test_url).expect("Failed to connect to test DB")
    }

    #[tokio::test]
    async fn test_save_loan_inserts_and_updates() {
        let mut conn = setup_test_db();

        use crate::schema::loans::dsl::*;
        diesel::delete(loans.filter(borrower.eq("TEST_BORROWER")))
            .execute(&mut conn)
            .unwrap();

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
            .filter(borrower.eq("TEST_BORROWER"))
            .first::<Loan>(&mut conn)
            .unwrap();

        assert_eq!(saved.borrowed_amount, 120);
        assert_eq!(saved.unpaid_interest, 5);

        diesel::delete(loans.filter(borrower.eq("TEST_BORROWER")))
            .execute(&mut conn)
            .unwrap();
    }
}
