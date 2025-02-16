use crate::{error::LoanPoolError, pool};
use soroban_sdk::Env;

#[allow(dead_code)]
// These are mockup numbers that should be set to each pool based on the token.
pub const BASE_INTEREST_RATE: i128 = 200_000; // 2%
pub const INTEREST_RATE_AT_PANIC: i128 = 1_000_000; // 10%
pub const MAX_INTEREST_RATE: i128 = 3_000_000; // 30%
pub const PANIC_BASE_RATE: i128 = -17_000_000;

#[allow(dead_code, unused_variables)]

pub fn get_interest(e: Env) -> Result<i128, LoanPoolError> {
    let interest_rate_multiplier = pool::read_interest_rate_multiplier(&e)?;
    const PANIC_RATES_THRESHOLD: i128 = 90_000_000;
    let available = pool::read_available_balance(&e)?;
    let total = pool::read_total_balance(&e)?;

    if total > 0 {
        let slope_before_panic = (INTEREST_RATE_AT_PANIC
            .checked_sub(BASE_INTEREST_RATE)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        .checked_mul(10_000_000)
        .ok_or(LoanPoolError::OverOrUnderFlow)?
        .checked_div(PANIC_RATES_THRESHOLD)
        .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let slope_after_panic = (MAX_INTEREST_RATE
            .checked_sub(INTEREST_RATE_AT_PANIC)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        .checked_mul(10_000_000)
        .ok_or(LoanPoolError::OverOrUnderFlow)?
        .checked_div(
            100_000_000_i128
                .checked_sub(PANIC_RATES_THRESHOLD)
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )
        .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let ratio_of_balances = ((total
            .checked_sub(available)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        .checked_mul(100_000_000)
        .ok_or(LoanPoolError::OverOrUnderFlow)?)
        .checked_div(total)
        .ok_or(LoanPoolError::OverOrUnderFlow)?;

        if ratio_of_balances < PANIC_RATES_THRESHOLD {
            Ok((slope_before_panic
                .checked_mul(ratio_of_balances)
                .ok_or(LoanPoolError::OverOrUnderFlow)?)
            .checked_div(10_000_000)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_add(BASE_INTEREST_RATE)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_mul(interest_rate_multiplier)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        } else {
            Ok((slope_after_panic
                .checked_mul(ratio_of_balances)
                .ok_or(LoanPoolError::OverOrUnderFlow)?)
            .checked_div(10_000_000)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_add(PANIC_BASE_RATE)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_mul(interest_rate_multiplier)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        }
    } else {
        Ok(BASE_INTEREST_RATE
            .checked_mul(interest_rate_multiplier)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
    }
}
