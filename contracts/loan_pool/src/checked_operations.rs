use crate::error::LoanPoolError;

pub trait CheckedOps: Sized {
    fn cmul(self, rhs: Self) -> Result<Self, LoanPoolError>;
    fn cdiv(self, rhs: Self) -> Result<Self, LoanPoolError>;
    fn cadd(self, rhs: Self) -> Result<Self, LoanPoolError>;
    fn csub(self, rhs: Self) -> Result<Self, LoanPoolError>;
}

impl CheckedOps for i128 {
    fn cmul(self, rhs: Self) -> Result<Self, LoanPoolError> {
        self.checked_mul(rhs).ok_or(LoanPoolError::OverOrUnderFlow)
    }

    fn cdiv(self, rhs: Self) -> Result<Self, LoanPoolError> {
        self.checked_div(rhs).ok_or(LoanPoolError::OverOrUnderFlow)
    }

    fn cadd(self, rhs: Self) -> Result<Self, LoanPoolError> {
        self.checked_add(rhs).ok_or(LoanPoolError::OverOrUnderFlow)
    }
    fn csub(self, rhs: Self) -> Result<Self, LoanPoolError> {
        self.checked_sub(rhs).ok_or(LoanPoolError::OverOrUnderFlow)
    }
}
