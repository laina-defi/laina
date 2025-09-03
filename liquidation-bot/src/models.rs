use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Queryable, Selectable, Insertable, Clone, PartialEq, Debug)]
#[diesel(table_name = crate::schema::loans)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Loan {
    pub borrower_address: String,
    pub nonce: i64,
    pub borrowed_amount: i64,
    pub borrowed_from: String,
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

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct LoanId {
    pub borrower_address: String,
    pub nonce: i64,
}

impl Display for LoanId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.borrower_address, self.nonce)
    }
}

impl FromStr for LoanId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');
        let borrower_address = parts
            .next()
            .ok_or_else(|| "missing borrower address".to_string())?
            .to_string();
        let nonce_str = parts.next().ok_or_else(|| "missing nonce".to_string())?;
        let nonce = nonce_str
            .parse::<i64>()
            .map_err(|_| "invalid nonce".to_string())?;
        Ok(LoanId {
            borrower_address,
            nonce,
        })
    }
}
