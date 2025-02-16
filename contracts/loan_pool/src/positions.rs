use crate::{
    error::LoanPoolError,
    storage_types::{extend_persistent, PoolDataKey, Positions},
};
use soroban_sdk::{Address, Env, IntoVal, Val};

pub fn read_positions(e: &Env, addr: &Address) -> Positions {
    let key = PoolDataKey::Positions(addr.clone());
    // If positions exist, read them and return them as Val
    if let Some(positions) = e.storage().persistent().get(&key) {
        extend_persistent(e.clone(), &key);
        positions
    } else {
        Positions {
            receivable_shares: 0,
            liabilities: 0,
            collateral: 0,
        }
    }
}

fn write_positions(e: &Env, addr: Address, receivables: i128, liabilities: i128, collateral: i128) {
    let key: PoolDataKey = PoolDataKey::Positions(addr);

    let positions: Positions = Positions {
        receivable_shares: receivables,
        liabilities,
        collateral,
    };

    let val: Val = positions.into_val(e);

    e.storage().persistent().set(&key, &val);

    extend_persistent(e.clone(), &key);
}

pub fn increase_positions(
    e: &Env,
    addr: Address,
    receivables: i128,
    liabilities: i128,
    collateral: i128,
) -> Result<(), LoanPoolError> {
    let positions = read_positions(e, &addr);

    let receivables_now: i128 = positions.receivable_shares;
    let liabilities_now: i128 = positions.liabilities;
    let collateral_now = positions.collateral;
    write_positions(
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
    let positions = read_positions(e, &addr);

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
    write_positions(
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
