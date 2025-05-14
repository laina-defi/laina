use core::time;
use log::{error, info, warn};
use std::thread;

use self::models::*;
use diesel::prelude::*;
use liquidation_bot::*;
use stellar_rpc_client::{self, Error, EventStart, EventType, GetEventsResponse};

const SLEEP_TIME_SECONDS: u64 = 10;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // let connection = &mut establish_connection();
    env_logger::init();

    info!("This is an info message");
    warn!("This is a warning message");
    error!("This is an error message");

    loop {
        fetch_events().await?;
        // get_prices();
        // find_liquidateable(connection);
        // attempt_liquidating();

        info!("Sleeping for {SLEEP_TIME_SECONDS} seconds.");
        thread::sleep(time::Duration::from_secs(SLEEP_TIME_SECONDS))
    }
}

async fn fetch_events() -> Result<GetEventsResponse, Error> {
    // TODO: fetch loans from Loan Manager
    // TODO: push new loans to the DB.
    info!("Fetching new loans from Loan Manager.");
    let url = "http://localhost:8000/soroban/rpc";
    let client = stellar_rpc_client::Client::new(url)?;

    let start = EventStart::Ledger(958657);
    let event_type = Some(EventType::Contract);
    let contract_ids = vec!["CD5QTKZZCIBF2LRGKWXB4KCRFDZAOBBLSS443FX3UODCATS2N27DWZMF".to_string()];
    let topics_slice = &[];
    let limit = Some(100);
    let events_response = client
        .get_events(start, event_type, &contract_ids, topics_slice, limit)
        .await?;
    println!("{:?}", events_response.events);
    Ok(events_response)
}

fn find_loans_from_events(events: GetEventsResponse) -> Result<Vec<Loan>, Error> {}

//
// fn get_prices() {
//     // TODO: fetch and return token prices from CoinGecko
//     info!("Getting prices from CoinGecko.")
// }
//
// fn find_liquidateable(connection: &mut PgConnection /*prices: Prices*/) {
//     use self::schema::loans::dsl::*;
//
//     let results = loans
//         .limit(5)
//         .select(Loan::as_select())
//         .load(connection)
//         .expect("Error loading loans");
//
//     info!("Displaying {} loans.", results.len());
//     for loan in results {
//         info!("{}", loan.id);
//     }
//
//     // TODO: calculate the health of each loan and return the unhealthy ones
// }
//
// fn attempt_liquidating(/* unhealthy_loans: Vec<Loan> */) {
//     // TODO: attempt to liquidate unhealthy loans
//     // TODO: update the loan in DB with the new values
// }
