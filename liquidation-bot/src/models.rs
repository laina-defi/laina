use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, Clone, PartialEq, Debug)]
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

#[derive(Queryable, Selectable, Insertable, Clone, PartialEq, Debug)]
#[diesel(table_name = crate::schema::prices)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Price {
    pub id: i32,
    pub pool_address: String,
    pub time_weighted_average_price: i64,
}
