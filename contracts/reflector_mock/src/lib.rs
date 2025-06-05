#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Asset {
    Stellar(Address),
    Other(Symbol),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceData {
    // The price in contracts' base asset and decimals.
    pub price: i128,
    // The timestamp of the price.
    pub timestamp: u64,
}

#[contract]
pub struct MockPriceOracleContract;

#[contractimpl]
impl MockPriceOracleContract {
    pub fn lastprice(_e: Env, _asset: Asset) -> Option<PriceData> {
        Some(PriceData {
            price: 1,
            timestamp: 1,
        })
    }
    pub fn twap(_e: Env, _asset: Asset, _records: u32) -> Option<i128> {
        Some(1)
    }
}
