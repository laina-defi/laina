use crate::checked_operations::CheckedOps;
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
        let slope_before_panic = (interest_rate_at_panic.csub(base_interest_rate)?)
            .cmul(DECIMAL)?
            .cdiv(panic_rates_threshold)?;

        let slope_after_panic = (max_interest_rate.csub(interest_rate_at_panic)?)
            .cmul(DECIMAL)?
            .cdiv(MAX_UTILIZATION.csub(panic_rates_threshold)?)?;

        let panic_base_rate =
            max_interest_rate.csub(slope_after_panic.cmul(MAX_UTILIZATION)?.cdiv(DECIMAL)?)?;

        let ratio_of_balances = ((total.csub(available)?).cmul(MAX_UTILIZATION)?).cdiv(total)?;

        if ratio_of_balances < panic_rates_threshold {
            Ok((slope_before_panic.cmul(ratio_of_balances)?)
                .cdiv(DECIMAL)?
                .cadd(base_interest_rate)?
                .cmul(interest_multiplier)?
                .cdiv(MULTIPLIER_DECIMAL)?)
        } else {
            Ok((slope_after_panic.cmul(ratio_of_balances)?)
                .cdiv(DECIMAL)?
                .cadd(panic_base_rate)?
                .cmul(interest_multiplier)?
                .cdiv(MULTIPLIER_DECIMAL)?)
        }
    } else {
        Ok(base_interest_rate
            .cmul(interest_multiplier)?
            .cdiv(MULTIPLIER_DECIMAL)?)
    }
}
