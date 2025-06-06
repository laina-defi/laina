use core::time;
use dotenvy::dotenv;
use log::{error, info, warn};
use soroban_sdk::xdr::{ScSymbol, StringM};
use std::collections::HashSet;
use std::{cell::RefCell, env, rc::Rc, str::FromStr, thread};
use stellar_xdr::curr::ScVal;

use self::models::*;
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
    transaction::{TransactionBehavior, TransactionBuilder, TransactionBuilderBehavior},
    Options,
};
use stellar_rpc_client::{self, Event, EventStart, EventType, GetEventsResponse};

const SLEEP_TIME_SECONDS: u64 = 10;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let connection = &mut establish_connection();
    env_logger::init();

    // TODO:Decide on how to handle the initial ledger
    let mut ledger = 1221500;

    loop {
        let GetEventsResponse {
            events,
            latest_ledger: new_ledger,
        } = fetch_events(ledger).await?;
        ledger = new_ledger;
        find_loans_from_events(events, connection).await?;
        fetch_prices(connection).await?;
        find_liquidateable(connection)?;

        info!("Sleeping for {SLEEP_TIME_SECONDS} seconds.");
        thread::sleep(time::Duration::from_secs(SLEEP_TIME_SECONDS))
    }
}

async fn fetch_events(ledger: u32) -> Result<GetEventsResponse, Error> {
    dotenv().ok();
    let loan_manager_id =
        env::var("CONTRACT_ID_LOAN_MANAGER").expect("CONTRACT_ID_LOAN_MANAGER must be set in .env");
    info!("Fetching new loans from Loan Manager.");
    let url =
        env::var("PUBLIC_SOROBAN_RPC_URL").expect("PUBLIC_SOROBAN_RPC_URL must be set in .env");
    let client = stellar_rpc_client::Client::new(&url)?;

    let start = EventStart::Ledger(ledger);
    let event_type = Some(EventType::Contract);
    let contract_ids = vec![loan_manager_id];
    let topics_slice = &[];
    let limit = Some(100);
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
    dotenv().ok();
    let url =
        env::var("PUBLIC_SOROBAN_RPC_URL").expect("PUBLIC_SOROBAN_RPC_URL must be set in .env");
    let server =
        soroban_client::Server::new(&url, Options::default()).expect("Cannot create server");

    // Load secret and derive public
    let secret_str =
        env::var("SOROBAN_SECRET_KEY").expect("SOROBAN_SECRET_KEY must be set in .env");
    let source_keypair = Keypair::from_secret(&secret_str).unwrap(); // Expected
    let source_public = source_keypair.public_key();

    let ledger_data = server.get_latest_ledger().await?;
    let source_account = Rc::new(RefCell::new(
        Account::new(&source_public, &ledger_data.sequence.to_string())
            .map_err(|e| anyhow::anyhow!("Account::new failed: {}", e))?,
    ));

    // Build operation
    let loan_manager_id =
        env::var("CONTRACT_ID_LOAN_MANAGER").expect("CONTRACT_ID_LOAN_MANAGER must be set in .env");
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
    let mut builder = TransactionBuilder::new(source_account.clone(), Networks::testnet(), None);
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

    let borrowed_from_uniques = loans.select(borrowed_from).distinct().load(connection)?;
    let collateral_from_uniques = loans.select(collateral_from).distinct().load(connection)?;
    let unique_pools_in_use: HashSet<String> = borrowed_from_uniques
        .into_iter()
        .chain(collateral_from_uniques)
        .collect();

    dotenv().ok();
    let url =
        env::var("PUBLIC_SOROBAN_RPC_URL").expect("PUBLIC_SOROBAN_RPC_URL must be set in .env");
    let server =
        soroban_client::Server::new(&url, Options::default()).expect("Cannot create server");
    let secret_str =
        env::var("SOROBAN_SECRET_KEY").expect("SOROBAN_SECRET_KEY must be set in .env");
    let source_keypair = Keypair::from_secret(&secret_str).unwrap(); // Expected
    let source_public = source_keypair.public_key();

    let ledger_data = server.get_latest_ledger().await?;
    let source_account = Rc::new(RefCell::new(
        Account::new(&source_public, &ledger_data.sequence.to_string())
            .map_err(|e| anyhow::anyhow!("Account::new failed: {}", e))?,
    ));

    // Build operation
    let oracle_contract_address =
        env::var("ORACLE_ADDRESS").expect("ORACLE_ADDRESS must be set in .env");
    let method = "twap"; // Time-weighted average price.

    let amount_of_data_points = ScVal::U32(12); // 12 datapoints with 5 min resolution -> average
                                                // of one hour

    for pool in unique_pools_in_use {
        // Temporary
        let currency = match pool.as_str() {
            "CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP" => "XLM",
            "CAXTXTUCA6ILFHCPIN34TWWVL4YL2QDDHYI65MVVQCEMDANFZLXVIEIK" => "USDC",
            "CDUFMIS6ZH3JM5MPNTWMDLBXPNQYV5FBPBGCFT2WWG4EXKGEPOCBNGCZ" => "EURC",
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
        let mut builder =
            TransactionBuilder::new(source_account.clone(), Networks::testnet(), None);
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

fn find_liquidateable(connection: &mut PgConnection) -> Result<(), Error> {
    use self::schema::loans::dsl::*;
    use self::schema::prices::dsl::*;

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
            borrowed_amount: amount_borrowed,
            borrowed_from: ref borrow_pool,
            collateral_amount: amount_collateral,
            collateral_from: ref collateral_pool,
            ..
        } = loan;

        let borrow_token_price = all_prices
            .iter()
            .find(|p| p.pool_address == *borrow_pool)
            .map(|p| p.time_weighted_average_price)
            .expect("No price found for borrow pool");

        let collateral_token_price = all_prices
            .iter()
            .find(|p| p.pool_address == *collateral_pool)
            .map(|p| p.time_weighted_average_price)
            .expect("No price found for collateral pool");

        // TODO:Figure out where we get this from. We have getter for it and pool address in this
        // scope
        let collateral_factor = 8000000;
        const DECIMAL_TO_INT_MULTIPLIER: i64 = 10_000_000;

        let collateral_value = collateral_token_price
            .checked_mul(amount_collateral)
            .ok_or(Error::msg("OverOrUnderFlow"))?
            .checked_mul(collateral_factor)
            .ok_or(Error::msg("OverOrUnderFlow"))?
            .checked_div(DECIMAL_TO_INT_MULTIPLIER)
            .ok_or(Error::msg("OverOrUnderFlow"))?;

        let borrowed_value = borrow_token_price
            .checked_mul(amount_borrowed)
            .ok_or(Error::msg("OverOrUnderFlow"))?;

        let health_factor = collateral_value
            .checked_mul(DECIMAL_TO_INT_MULTIPLIER)
            .ok_or(Error::msg("OverOrUnderFlow"))?
            .checked_div(borrowed_value)
            .ok_or(Error::msg("OverOrUnderFlow"))?;

        let health_factor_threshold = 10_100_000;
        if health_factor < health_factor_threshold {
            attempt_liquidating(loan);
        }
    }

    Ok(())
}

fn attempt_liquidating(loan: Loan) {
    // TODO: attempt to liquidate unhealthy loans
    // TODO: update the loan in DB with the new values
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
