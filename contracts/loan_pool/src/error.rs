use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LoanPoolError {
    LoanManager = 1,
    Currency = 2,
    LiquidationThreshold = 3,
    TotalShares = 4,
    TotalBalance = 5,
    AvailableBalance = 6,
    Accrual = 7,
    AccrualLastUpdated = 8,
    OverOrUnderFlow = 9,
    NegativeDeposit = 10,
    WithdrawOverBalance = 11,
    WithdrawIsNegative = 12,
    InterestRateMultiplier = 13,
    PoolStatus = 14,
    WrongStatus = 15,
}
