use crate::{error::LoanPoolError, storage};
use soroban_sdk::{Address, Env};

pub fn increase_positions(
    e: &Env,
    addr: Address,
    receivables: i128,
    liabilities: i128,
    collateral: i128,
) -> Result<(), LoanPoolError> {
    let positions = storage::read_positions(e, &addr);

    let receivables_now: i128 = positions.receivable_shares;
    let liabilities_now: i128 = positions.liabilities;
    let collateral_now = positions.collateral;
    storage::write_positions(
        e,
        addr,
        receivables_now
            .checked_add(receivables)
            .ok_or(LoanPoolError::OverOrUnderFlow)?,
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
    receivables: i128,
    liabilities: i128,
    collateral: i128,
) -> Result<(), LoanPoolError> {
    let positions = storage::read_positions(e, &addr);

    // TODO: Might need to use get rather than get_unchecked and convert from Option<V> to V
    let receivables_now = positions.receivable_shares;
    let liabilities_now = positions.liabilities;
    let collateral_now = positions.collateral;

    if receivables_now < receivables {
        panic!("insufficient receivables");
    }
    if liabilities_now < liabilities {
        panic!("insufficient liabilities");
    }
    if collateral_now < collateral {
        panic!("insufficient collateral");
    }
    storage::write_positions(
        e,
        addr,
        receivables_now
            .checked_sub(receivables)
            .ok_or(LoanPoolError::OverOrUnderFlow)?,
        liabilities_now
            .checked_sub(liabilities)
            .ok_or(LoanPoolError::OverOrUnderFlow)?,
        collateral_now
            .checked_sub(collateral)
            .ok_or(LoanPoolError::OverOrUnderFlow)?,
    );
    Ok(())
}
