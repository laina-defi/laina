use crate::error::LoanManagerError;

pub trait CheckedOps: Sized {
    fn cmul(self, rhs: Self) -> Result<Self, LoanManagerError>;
    fn cdiv(self, rhs: Self) -> Result<Self, LoanManagerError>;
    fn cadd(self, rhs: Self) -> Result<Self, LoanManagerError>;
    fn csub(self, rhs: Self) -> Result<Self, LoanManagerError>;
}

impl CheckedOps for i128 {
    fn cmul(self, rhs: Self) -> Result<Self, LoanManagerError> {
        self.checked_mul(rhs)
            .ok_or(LoanManagerError::OverOrUnderFlow)
    }

    fn cdiv(self, rhs: Self) -> Result<Self, LoanManagerError> {
        self.checked_div(rhs)
            .ok_or(LoanManagerError::OverOrUnderFlow)
    }

    fn cadd(self, rhs: Self) -> Result<Self, LoanManagerError> {
        self.checked_add(rhs)
            .ok_or(LoanManagerError::OverOrUnderFlow)
    }
    fn csub(self, rhs: Self) -> Result<Self, LoanManagerError> {
        self.checked_sub(rhs)
            .ok_or(LoanManagerError::OverOrUnderFlow)
    }
}
