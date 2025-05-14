use core::time;
use log::{error, info, warn};
use std::thread;

use self::models::*;
use diesel::prelude::*;
use liquidation_bot::*;
use stellar_rpc_client;
use tokio;

const SLEEP_TIME_SECONDS: u64 = 10;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let connection = &mut establish_connection();
    env_logger::init();

    info!("This is an info message");
    warn!("This is a warning message");
    error!("This is an error message");

    loop {
        get_new_loans();
        get_prices();
        find_liquidateable(connection);
        attempt_liquidating();

        info!("Sleeping for {SLEEP_TIME_SECONDS} seconds.");
        thread::sleep(time::Duration::from_secs(SLEEP_TIME_SECONDS))
    }
}

async fn get_new_loans() {
    // TODO: fetch loans from Loan Manager
    // TODO: push new loans to the DB.
    info!("Fetching new loans from Loan Manager.");
    let url = "http://localhost:8000/soroban/rpc";
    let client = stellar_rpc_client::Client::new(url)?;
    let health = client.get_health().await;
    println!(health);
}

fn get_prices() {
    // TODO: fetch and return token prices from CoinGecko
    info!("Getting prices from CoinGecko.")
}

fn find_liquidateable(connection: &mut PgConnection /*prices: Prices*/) {
    use self::schema::loans::dsl::*;

    let results = loans
        .limit(5)
        .select(Loan::as_select())
        .load(connection)
        .expect("Error loading loans");

    info!("Displaying {} loans.", results.len());
    for loan in results {
        info!("{}", loan.id);
    }

    // TODO: calculate the health of each loan and return the unhealthy ones
}

fn attempt_liquidating(/* unhealthy_loans: Vec<Loan> */) {
    // TODO: attempt to liquidate unhealthy loans
    // TODO: update the loan in DB with the new values
}
