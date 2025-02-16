use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LoanManagerError {
    AlreadyInitialized = 1,
    LoanAlreadyExists = 2,
    AdminNotFound = 3,
    OverOrUnderFlow = 4,
    NoLastPrice = 5,
    AddressNotFound = 6,
    LoanNotFound = 7,
}
