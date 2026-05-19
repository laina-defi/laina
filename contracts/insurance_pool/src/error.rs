use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InsurancePoolError {
    LoanPoolAddress = 1,
    ShareTokenAddress = 2,
    InsufficientInsuranceTokens = 3,
    InsufficientInsuranceShares = 4,
    OverOrUnderFlow = 5,
    NegativeAmount = 6,
    ZeroTotalShares = 7,
    QueuePeriodNotElapsed = 8,
    NoWithdrawQueue = 9,
    WithdrawQueueAlreadyExists = 10,
    AlreadyInitialized = 11,
}
