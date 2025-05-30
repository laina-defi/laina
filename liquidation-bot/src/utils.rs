use std::collections::HashMap;
use std::str::FromStr;

use crate::models::Loan;
use anyhow::{anyhow, Error, Result};
use base64::engine::general_purpose::STANDARD as base64_engine;
use base64::Engine;
use soroban_client::address::{Address, AddressTrait};
use soroban_client::xdr::int128_helpers::i128_from_pieces;
use soroban_client::xdr::{Int128Parts, ScSymbol, ScVal, StringM, VecM};
use stellar_xdr::curr::{Limits, ReadXdr, SorobanAuthorizationEntry};

pub enum Asset {
    Stellar(Address),
    Other(ScSymbol),
}

pub fn asset_to_scval(value: &Asset) -> Result<ScVal, Error> {
    match value {
        Asset::Stellar(address) => {
            let vec = vec![
                ScVal::Symbol(ScSymbol(StringM::from_str("Stellar").unwrap())),
                address
                    .to_sc_val()
                    .map_err(|e| anyhow!("Address.to_sc_val failed: {e}"))?,
            ];
            let vecm: VecM<ScVal, { u32::MAX }> = vec
                .try_into()
                .map_err(|_| anyhow!("Failed to convert Vec to VecM"))?;

            Ok(ScVal::Vec(Some(vecm.into())))
        }
        Asset::Other(ticker) => {
            let vec = vec![
                ScVal::Symbol(ScSymbol(StringM::from_str("Other").unwrap())),
                ScVal::Symbol(ticker.clone()),
            ];
            let vecm: VecM<ScVal, { u32::MAX }> = vec
                .try_into()
                .map_err(|_| anyhow!("Failed to convert Vec to VecM"))?;

            Ok(ScVal::Vec(Some(vecm.into())))
        }
    }
}

pub fn decode_loan_from_simulate_response(
    result: (ScVal, Vec<SorobanAuthorizationEntry>),
) -> Result<Loan, Error> {
    let map = extract_map(&result.0).unwrap();

    let borrower_value =
        scval_to_address_string(map.get("borrower").ok_or(Error::msg("no key found"))?)?;
    let borrowed_from_value =
        scval_to_address_string(map.get("borrowed_from").ok_or(Error::msg("no key found"))?)?;
    let borrowed_amount_value = scval_to_i128(
        map.get("borrowed_amount")
            .ok_or(Error::msg("no key found"))?,
    )? as i64;
    let collateral_amount_value = scval_to_i128(
        map.get("collateral_amount")
            .ok_or(Error::msg("no key found"))?,
    )? as i64;
    let collateral_from_value = scval_to_address_string(
        map.get("collateral_from")
            .ok_or(Error::msg("no key found"))?,
    )?;
    let unpaid_interest_value = scval_to_i128(
        map.get("unpaid_interest")
            .ok_or(Error::msg("no key found"))?,
    )? as i64;

    let loan = Loan {
        borrower: borrower_value,
        borrowed_from: borrowed_from_value,
        id: 1,
        borrowed_amount: borrowed_amount_value as i64,
        collateral_amount: collateral_amount_value,
        collateral_from: collateral_from_value,
        unpaid_interest: unpaid_interest_value,
    };

    Ok(loan)
}

pub fn scval_to_i128(val: &ScVal) -> Result<i128> {
    match val {
        ScVal::I128(parts) => Ok(i128_from_pieces(parts.hi, parts.lo)),
        _ => Err(anyhow!("Expected ScVal::I128")),
    }
}

pub fn scval_to_address_string(val: &ScVal) -> Result<String> {
    match val {
        ScVal::Address(addr) => Ok(addr.to_string()),
        _ => Err(anyhow!("Expected ScVal::Address")),
    }
}

pub fn extract_map(scval: &ScVal) -> Result<HashMap<String, ScVal>> {
    match scval {
        ScVal::Map(Some(scmap)) => {
            let mut result = HashMap::new();

            for entry in scmap.0.as_slice() {
                let key = &entry.key;
                let val = &entry.val;

                if let ScVal::Symbol(sym) = key {
                    let key_bytes = sym.0.as_slice();
                    let key_str = std::str::from_utf8(key_bytes)
                        .map_err(|_| anyhow!("Invalid UTF-8 in symbol key"))?;
                    result.insert(key_str.to_string(), val.clone());
                } else {
                    return Err(anyhow!("Non-symbol key found in map"));
                }
            }

            Ok(result)
        }
        _ => Err(anyhow!("Expected ScVal::Map")),
    }
}

pub fn extract_i128_from_result(
    res: Option<(ScVal, Vec<SorobanAuthorizationEntry>)>,
) -> Option<i128> {
    res.and_then(|(scval, _auth)| {
        if let ScVal::I128(Int128Parts { hi, lo }) = scval {
            // Convert Int128Parts { hi, lo } to i128
            let combined = i128_from_pieces(hi, lo);
            Some(combined)
        } else {
            None
        }
    })
}

pub fn decode_topic(topic: Vec<String>) -> Result<Vec<String>, Error> {
    let mut decoded_topics = Vec::new();

    for string in topic {
        let decoded = base64_engine.decode(string)?;
        let scval = ScVal::from_xdr(
            decoded,
            Limits {
                depth: 64,
                len: 10000,
            },
        )?;

        if let ScVal::Symbol(symbol) = scval {
            decoded_topics.push(symbol.to_string());
        }
    }
    Ok(decoded_topics)
}

pub fn decode_value(value: String) -> Result<Vec<String>, Error> {
    let decoded = base64_engine.decode(value)?;
    let scval = ScVal::from_xdr(
        decoded,
        Limits {
            depth: 64,
            len: 10000,
        },
    )?;

    let vec = match scval {
        ScVal::Vec(Some(v)) => v,
        _ => return Err(anyhow::anyhow!("Expected ScVal::Vec")),
    };

    let mut result_parts = Vec::new();

    for item in vec.iter() {
        match item {
            ScVal::Symbol(symbol) => result_parts.push(symbol.to_string()),
            ScVal::Address(address) => result_parts.push(address.to_string()),
            other => result_parts.push(format!("{:?}", other)),
        }
    }

    Ok(result_parts)
}
