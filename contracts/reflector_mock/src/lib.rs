#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

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

#[contracttype]
pub enum DataKey {
    Price(Asset),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReflectorMockError {
    CannotSetPrice = 1,
}

#[contract]
pub struct MockPriceOracleContract;

#[contractimpl]
impl MockPriceOracleContract {
    pub fn lastprice(_e: Env, _asset: Asset) -> Option<PriceData> {
        _e.storage()
            .persistent()
            .get(&DataKey::Price(_asset))
            .or(Some(PriceData {
                price: 1,
                timestamp: 1,
            }))
    }

    pub fn twap(_e: Env, _asset: Asset, _records: u32) -> Option<i128> {
        _e.storage()
            .persistent()
            .get(&DataKey::Price(_asset))
            .map(|data: PriceData| data.price)
            .or(Some(1))
    }

    pub fn update_price(e: Env, asset: Asset, price: PriceData) -> Result<(), ReflectorMockError> {
        e.storage().persistent().set(&DataKey::Price(asset), &price);
        Ok(())
    }
}
