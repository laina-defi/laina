#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, token, Address, Env, Symbol};

const USDC: Symbol = symbol_short!("USDC");
const EURC: Symbol = symbol_short!("EURC");
const LAI: Symbol = symbol_short!("LAI");

// 10,000 tokens with 7 decimal places
const CLAIM_AMOUNT: i128 = 10_000 * 10_000_000;

#[contract]
struct FaucetContract;

#[contractimpl]
impl FaucetContract {
    pub fn initialize(e: Env, usdc: Address, eurc: Address, lai: Address) {
        e.storage().instance().set(&USDC, &usdc);
        e.storage().instance().set(&EURC, &eurc);
        e.storage().instance().set(&LAI, &lai);
    }

    pub fn claim(e: Env, to: Address) {
        to.require_auth();

        let me = e.current_contract_address();
        let usdc: Address = e.storage().instance().get(&USDC).unwrap();
        let eurc: Address = e.storage().instance().get(&EURC).unwrap();
        let lai: Address = e.storage().instance().get(&LAI).unwrap();

        token::Client::new(&e, &usdc).transfer(&me, &to, &CLAIM_AMOUNT);
        token::Client::new(&e, &eurc).transfer(&me, &to, &CLAIM_AMOUNT);
        token::Client::new(&e, &lai).transfer(&me, &to, &CLAIM_AMOUNT);
    }
}
