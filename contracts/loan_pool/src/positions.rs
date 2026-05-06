use crate::checked_operations::CheckedOps;
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
        liabilities_now.cadd(liabilities)?,
        collateral_now.cadd(collateral)?,
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
        liabilities_now.csub(liabilities)?,
        collateral_now.csub(collateral)?,
    );
    Ok(())
}
