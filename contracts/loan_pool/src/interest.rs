use crate::{
    error::LoanPoolError,
    storage::{self, InterestDto, DECIMAL},
};
use soroban_sdk::Env;

#[allow(dead_code)]
pub fn get_interest(e: Env) -> Result<i128, LoanPoolError> {
    let InterestDto {
        base_interest_rate,
        interest_rate_at_panic,
        max_interest_rate,
        panic_rates_threshold,
        interest_multiplier,
        ..
    } = storage::read_interest_dto(&e)?;
    let available = storage::read_available_balance(&e)?;
    let total = storage::read_total_balance(&e)?;

    const MAX_UTILIZATION: i128 = 100_000_000;
    const MULTIPLIER_DECIMAL: i128 = 100;

    if total > 0 {
        let slope_before_panic = (interest_rate_at_panic
            .checked_sub(base_interest_rate)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        .checked_mul(DECIMAL)
        .ok_or(LoanPoolError::OverOrUnderFlow)?
        .checked_div(panic_rates_threshold)
        .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let slope_after_panic = (max_interest_rate
            .checked_sub(interest_rate_at_panic)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        .checked_mul(DECIMAL)
        .ok_or(LoanPoolError::OverOrUnderFlow)?
        .checked_div(
            MAX_UTILIZATION
                .checked_sub(panic_rates_threshold)
                .ok_or(LoanPoolError::OverOrUnderFlow)?,
        )
        .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let panic_base_rate = max_interest_rate
            .checked_sub(
                slope_after_panic
                    .checked_mul(MAX_UTILIZATION)
                    .ok_or(LoanPoolError::OverOrUnderFlow)?
                    .checked_div(DECIMAL)
                    .ok_or(LoanPoolError::OverOrUnderFlow)?,
            )
            .ok_or(LoanPoolError::OverOrUnderFlow)?;

        let ratio_of_balances = ((total
            .checked_sub(available)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        .checked_mul(MAX_UTILIZATION)
        .ok_or(LoanPoolError::OverOrUnderFlow)?)
        .checked_div(total)
        .ok_or(LoanPoolError::OverOrUnderFlow)?;

        if ratio_of_balances < panic_rates_threshold {
            Ok((slope_before_panic
                .checked_mul(ratio_of_balances)
                .ok_or(LoanPoolError::OverOrUnderFlow)?)
            .checked_div(DECIMAL)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_add(base_interest_rate)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_mul(interest_multiplier)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(MULTIPLIER_DECIMAL)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        } else {
            Ok((slope_after_panic
                .checked_mul(ratio_of_balances)
                .ok_or(LoanPoolError::OverOrUnderFlow)?)
            .checked_div(DECIMAL)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_add(panic_base_rate)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_mul(interest_multiplier)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(MULTIPLIER_DECIMAL)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
        }
    } else {
        Ok(base_interest_rate
            .checked_mul(interest_multiplier)
            .ok_or(LoanPoolError::OverOrUnderFlow)?
            .checked_div(MULTIPLIER_DECIMAL)
            .ok_or(LoanPoolError::OverOrUnderFlow)?)
    }
}
