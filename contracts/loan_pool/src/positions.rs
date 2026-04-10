use crate::{error::LoanPoolError, storage};
use soroban_sdk::{Address, Env};

pub fn increase_positions(
    e: &Env,
    addr: Address,
    liabilities: i128,
    collateral: i128,
) -> Result<(), LoanPoolError> {
    let stored = storage::read_stored_positions(e, &addr);

    let liabilities_now: i128 = stored.liabilities;
    let collateral_now = stored.collateral_shares;
    storage::write_positions(
        e,
        addr,
        liabilities_now
            .checked_add(liabilities)
            .ok_or(LoanPoolError::OverOrUnderFlow)?,
        collateral_now
            .checked_add(collateral)
            .ok_or(LoanPoolError::OverOrUnderFlow)?,
    );
    Ok(())
}

pub fn decrease_positions(
    e: &Env,
    addr: Address,
    liabilities: i128,
    collateral: i128,
) -> Result<(), LoanPoolError> {
    let stored = storage::read_stored_positions(e, &addr);

    let liabilities_now = stored.liabilities;
    let collateral_now = stored.collateral_shares;

    if liabilities_now < liabilities {
        panic!("insufficient liabilities");
    }
    if collateral_now < collateral {
        panic!("insufficient collateral");
    }
    storage::write_positions(
        e,
        addr,
        liabilities_now
            .checked_sub(liabilities)
            .ok_or(LoanPoolError::OverOrUnderFlow)?,
        collateral_now
            .checked_sub(collateral)
            .ok_or(LoanPoolError::OverOrUnderFlow)?,
    );
    Ok(())
}
