use diesel::prelude::*;
use serde::Deserialize;

#[derive(Queryable, Selectable, Deserialize, Debug)]
#[diesel(table_name = crate::schema::loans)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Loan {
    pub id: i32,
    pub borrowed_amount: i64,
    pub borrowed_from: String,
    pub borrower: String,
    pub collateral_amount: i64,
    pub collateral_from: String,
    pub unpaid_interest: i64,
}
