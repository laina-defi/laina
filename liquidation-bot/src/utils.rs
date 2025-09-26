use std::collections::HashMap;
use std::str::FromStr;

use crate::models::{Loan, LoanId};
use anyhow::{anyhow, Error, Result};
use base64::engine::general_purpose::STANDARD as base64_engine;
use base64::Engine;
use soroban_client::address::{Address, AddressTrait};
use soroban_client::xdr::int128_helpers::i128_from_pieces;
use soroban_client::xdr::{
    Int128Parts, Limits, ReadXdr, ScSymbol, ScVal, SorobanAuthorizationEntry, StringM, VecM,
};

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

/// Parses loan ID from deleted loan event topic
/// The topic structure is: ["loan_deleted", LoanId] where LoanId is a Map with borrower_address and nonce
pub fn parse_loan_id_from_topic(topic: &[ScVal]) -> Result<crate::models::LoanId, Error> {
    if topic.len() != 2 {
        return Err(Error::msg("Expected topic to have exactly 2 elements"));
    }

    // First element should be "loan_deleted"
    let event_type = &topic[0];
    match event_type {
        ScVal::Symbol(symbol) => {
            if symbol.to_string() != "loan_deleted" {
                return Err(Error::msg(
                    "Expected first topic element to be 'loan_deleted'",
                ));
            }
        }
        _ => return Err(Error::msg("Expected first topic element to be a symbol")),
    }

    // Second element should be the LoanId map
    let loan_id_map = extract_map(&topic[1])?;

    let borrower_address = scval_to_address_string(
        loan_id_map
            .get("borrower_address")
            .ok_or(Error::msg("borrower_address not found in loan_id"))?,
    )?;

    let nonce_val = loan_id_map
        .get("nonce")
        .ok_or(Error::msg("nonce not found in loan_id"))?;

    let nonce = match nonce_val {
        ScVal::U64(n) => *n as i64,
        _ => return Err(Error::msg("nonce is not a U64")),
    };

    Ok(crate::models::LoanId {
        borrower_address,
        nonce,
    })
}

/// Parses loan data from RPC event response format
/// The data structure is: Map(Some(ScMap(VecM([ScMapEntry { key: Symbol("loan"), val: Map(...) }])))
pub fn parse_loan_from_rpc_event(event_value: &ScVal) -> Result<Loan, Error> {
    let outer_map = extract_map(event_value)?;

    let loan_map_val = outer_map
        .get("loan")
        .ok_or(Error::msg("loan key not found in outer map"))?;

    let loan_map = extract_map(loan_map_val)?;

    let loan_id_val = loan_map
        .get("loan_id")
        .ok_or(Error::msg("loan_id not found in loan map"))?;
    let loan_id_map = extract_map(loan_id_val)?;

    let borrower_address = scval_to_address_string(
        loan_id_map
            .get("borrower_address")
            .ok_or(Error::msg("borrower_address not found in loan_id"))?,
    )?;

    let nonce = match loan_id_map.get("nonce") {
        Some(ScVal::U64(n)) => *n as i64,
        _ => return Err(Error::msg("nonce not found or invalid type in loan_id")),
    };

    let borrowed_amount = scval_to_i128(
        loan_map
            .get("borrowed_amount")
            .ok_or(Error::msg("borrowed_amount not found"))?,
    )? as i64;

    let borrowed_from = scval_to_address_string(
        loan_map
            .get("borrowed_from")
            .ok_or(Error::msg("borrowed_from not found"))?,
    )?;

    let collateral_amount = scval_to_i128(
        loan_map
            .get("collateral_amount")
            .ok_or(Error::msg("collateral_amount not found"))?,
    )? as i64;

    let collateral_from = scval_to_address_string(
        loan_map
            .get("collateral_from")
            .ok_or(Error::msg("collateral_from not found"))?,
    )?;

    let unpaid_interest = scval_to_i128(
        loan_map
            .get("unpaid_interest")
            .ok_or(Error::msg("unpaid_interest not found"))?,
    )? as i64;

    Ok(Loan {
        borrower_address,
        nonce,
        borrowed_amount,
        borrowed_from,
        collateral_amount,
        collateral_from,
        unpaid_interest,
    })
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

pub fn decode_loan_event(value: String) -> Result<LoanId, Error> {
    let decoded = base64_engine.decode(value)?;
    let scval = ScVal::from_xdr(
        decoded,
        Limits {
            depth: 64,
            len: 10000,
        },
    )?;

    let map = extract_map(&scval)?;

    let borrower_address = scval_to_address_string(
        map.get("borrower_address")
            .ok_or(Error::msg("borrower_address not found"))?,
    )?;

    let nonce = match map.get("nonce") {
        Some(ScVal::U64(n)) => *n as i64,
        _ => return Err(Error::msg("nonce not found or invalid type")),
    };

    Ok(LoanId {
        borrower_address,
        nonce,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_client::xdr::{Int128Parts, ScAddress, ScMapEntry};
    use std::str::FromStr;
    use stellar_xdr::curr::ScMap;

    #[test]
    fn scval_to_i128_success() {
        let parts = Int128Parts { hi: 0, lo: 1000 };
        let scval = ScVal::I128(parts);

        let result = scval_to_i128(&scval).unwrap();
        assert_eq!(result, 1000);
    }

    #[test]
    fn scval_to_i128_negative() {
        let parts = Int128Parts {
            hi: -1,
            lo: u64::MAX - 999,
        };
        let scval = ScVal::I128(parts);

        let result = scval_to_i128(&scval).unwrap();
        assert_eq!(result, -1000);
    }

    #[test]
    fn scval_to_i128_wrong_type() {
        let scval = ScVal::U32(100);
        let result = scval_to_i128(&scval);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Expected ScVal::I128");
    }

    #[test]
    fn scval_to_address_string_success() {
        let address_str = "CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP";
        let address = ScAddress::from_str(address_str).unwrap();
        let scval = ScVal::Address(address);

        let result = scval_to_address_string(&scval).unwrap();
        assert_eq!(result, address_str);
    }

    #[test]
    fn scval_to_address_string_wrong_type() {
        let scval = ScVal::U32(100);
        let result = scval_to_address_string(&scval);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Expected ScVal::Address");
    }

    #[test]
    fn extract_map_success() {
        let mut entries = Vec::new();
        entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("borrower").unwrap())),
            val: ScVal::U32(1000),
        });
        entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("amount").unwrap())),
            val: ScVal::U32(5000),
        });

        let scmap = ScMap(entries.try_into().unwrap());
        let scval = ScVal::Map(Some(scmap));

        let result = extract_map(&scval).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("borrower"));
        assert!(result.contains_key("amount"));
    }

    #[test]
    fn extract_map_empty() {
        let entries = Vec::new();
        let scmap = ScMap(entries.try_into().unwrap());
        let scval = ScVal::Map(Some(scmap));

        let result = extract_map(&scval).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn extract_map_wrong_type() {
        let scval = ScVal::U32(100);
        let result = extract_map(&scval);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Expected ScVal::Map");
    }

    #[test]
    fn extract_map_none() {
        let scval = ScVal::Map(None);
        let result = extract_map(&scval);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Expected ScVal::Map");
    }

    #[test]
    fn extract_i128_from_result_success() {
        let parts = Int128Parts { hi: 0, lo: 2500 };
        let scval = ScVal::I128(parts);
        let result = Some((scval, Vec::new()));

        let extracted = extract_i128_from_result(result);
        assert_eq!(extracted, Some(2500));
    }

    #[test]
    fn extract_i128_from_result_none() {
        let result = extract_i128_from_result(None);
        assert_eq!(result, None);
    }

    #[test]
    fn extract_i128_from_result_wrong_type() {
        let scval = ScVal::U32(100);
        let result = Some((scval, Vec::new()));

        let extracted = extract_i128_from_result(result);
        assert_eq!(extracted, None);
    }

    #[test]
    fn extract_i128_from_result_large_number() {
        let parts = Int128Parts { hi: 1, lo: 0 };
        let scval = ScVal::I128(parts);
        let result = Some((scval, Vec::new()));

        let extracted = extract_i128_from_result(result);
        let expected = i128_from_pieces(1, 0);
        assert_eq!(extracted, Some(expected));
    }

    #[test]
    fn asset_to_scval_other() {
        let asset = Asset::Other(ScSymbol(StringM::from_str("USDC").unwrap()));
        let result = asset_to_scval(&asset).unwrap();

        if let ScVal::Vec(Some(vec)) = result {
            assert_eq!(vec.len(), 2);
            if let ScVal::Symbol(symbol) = &vec[0] {
                assert_eq!(symbol.to_string(), "Other");
            } else {
                panic!("Expected Symbol for variant");
            }
            if let ScVal::Symbol(symbol) = &vec[1] {
                assert_eq!(symbol.to_string(), "USDC");
            } else {
                panic!("Expected Symbol for asset");
            }
        } else {
            panic!("Expected ScVal::Vec");
        }
    }

    #[test]
    fn asset_to_scval_stellar() {
        let address =
            Address::from_string("CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP")
                .unwrap();
        let asset = Asset::Stellar(address);
        let result = asset_to_scval(&asset).unwrap();

        if let ScVal::Vec(Some(vec)) = result {
            assert_eq!(vec.len(), 2);
            if let ScVal::Symbol(symbol) = &vec[0] {
                assert_eq!(symbol.to_string(), "Stellar");
            } else {
                panic!("Expected Symbol for variant");
            }
            if let ScVal::Address(_) = &vec[1] {
                // Address conversion successful
            } else {
                panic!("Expected Address for asset");
            }
        } else {
            panic!("Expected ScVal::Vec");
        }
    }

    #[test]
    fn decode_topic_invalid_base64() {
        let invalid_topic = vec!["invalid_base64".to_string()];
        let result = decode_topic(invalid_topic);
        assert!(result.is_err());
    }

    #[test]
    fn decode_value_invalid_base64() {
        let invalid_value = "invalid_base64".to_string();
        let result = decode_value(invalid_value);
        assert!(result.is_err());
    }

    #[test]
    fn parse_loan_from_rpc_event_success() {
        // Create loan_id map
        let mut loan_id_entries = Vec::new();
        loan_id_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("borrower_address").unwrap())),
            val: ScVal::Address(
                ScAddress::from_str("CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP")
                    .unwrap(),
            ),
        });
        loan_id_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("nonce").unwrap())),
            val: ScVal::U64(3),
        });
        let loan_id_map = ScMap(loan_id_entries.try_into().unwrap());

        // Create loan map
        let mut loan_entries = Vec::new();
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("borrowed_amount").unwrap())),
            val: ScVal::I128(Int128Parts {
                hi: 0,
                lo: 282333967,
            }),
        });
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("borrowed_from").unwrap())),
            val: ScVal::Address(
                ScAddress::from_str("CAXTXTUCA6ILFHCPIN34TWWVL4YL2QDDHYI65MVVQCEMDANFZLXVIEIK")
                    .unwrap(),
            ),
        });
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("collateral_amount").unwrap())),
            val: ScVal::I128(Int128Parts {
                hi: 0,
                lo: 136658653,
            }),
        });
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("collateral_from").unwrap())),
            val: ScVal::Address(
                ScAddress::from_str("CDUFMIS6ZH3JM5MPNTWMDLBXPNQYV5FBPBGCFT2WWG4EXKGEPOCBNGCZ")
                    .unwrap(),
            ),
        });
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("health_factor").unwrap())),
            val: ScVal::I128(Int128Parts {
                hi: 0,
                lo: 11922149,
            }),
        });
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("last_accrual").unwrap())),
            val: ScVal::I128(Int128Parts {
                hi: 0,
                lo: 10003568,
            }),
        });
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("loan_id").unwrap())),
            val: ScVal::Map(Some(loan_id_map)),
        });
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("unpaid_interest").unwrap())),
            val: ScVal::I128(Int128Parts { hi: 0, lo: 0 }),
        });
        let loan_map = ScMap(loan_entries.try_into().unwrap());

        // Create outer map
        let mut outer_entries = Vec::new();
        outer_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("loan").unwrap())),
            val: ScVal::Map(Some(loan_map)),
        });
        let outer_map = ScMap(outer_entries.try_into().unwrap());
        let event_value = ScVal::Map(Some(outer_map));

        let loan = parse_loan_from_rpc_event(&event_value).unwrap();

        assert_eq!(
            loan.borrower_address,
            "CCDF2NOJXOW73SXXB6BZRAPGVNJU7VMUURXCVLRHCHHAXHOY2TVRLFFP"
        );
        assert_eq!(loan.nonce, 3);
        assert_eq!(loan.borrowed_amount, 282333967);
        assert_eq!(
            loan.borrowed_from,
            "CAXTXTUCA6ILFHCPIN34TWWVL4YL2QDDHYI65MVVQCEMDANFZLXVIEIK"
        );
        assert_eq!(loan.collateral_amount, 136658653);
        assert_eq!(
            loan.collateral_from,
            "CDUFMIS6ZH3JM5MPNTWMDLBXPNQYV5FBPBGCFT2WWG4EXKGEPOCBNGCZ"
        );
        assert_eq!(loan.unpaid_interest, 0);
    }

    #[test]
    fn parse_loan_from_rpc_event_missing_loan_key() {
        let mut outer_entries = Vec::new();
        outer_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("other_key").unwrap())),
            val: ScVal::U32(100),
        });
        let outer_map = ScMap(outer_entries.try_into().unwrap());
        let event_value = ScVal::Map(Some(outer_map));

        let result = parse_loan_from_rpc_event(&event_value);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("loan key not found"));
    }

    #[test]
    fn parse_loan_from_rpc_event_missing_loan_id() {
        let mut loan_entries = Vec::new();
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("borrowed_amount").unwrap())),
            val: ScVal::I128(Int128Parts { hi: 0, lo: 1000 }),
        });
        let loan_map = ScMap(loan_entries.try_into().unwrap());

        let mut outer_entries = Vec::new();
        outer_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("loan").unwrap())),
            val: ScVal::Map(Some(loan_map)),
        });
        let outer_map = ScMap(outer_entries.try_into().unwrap());
        let event_value = ScVal::Map(Some(outer_map));

        let result = parse_loan_from_rpc_event(&event_value);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("loan_id not found"));
    }

    #[test]
    fn parse_loan_from_rpc_event_missing_borrower_address() {
        let mut loan_id_entries = Vec::new();
        loan_id_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("nonce").unwrap())),
            val: ScVal::U64(3),
        });
        let loan_id_map = ScMap(loan_id_entries.try_into().unwrap());

        let mut loan_entries = Vec::new();
        loan_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("loan_id").unwrap())),
            val: ScVal::Map(Some(loan_id_map)),
        });
        let loan_map = ScMap(loan_entries.try_into().unwrap());

        let mut outer_entries = Vec::new();
        outer_entries.push(ScMapEntry {
            key: ScVal::Symbol(ScSymbol(StringM::from_str("loan").unwrap())),
            val: ScVal::Map(Some(loan_map)),
        });
        let outer_map = ScMap(outer_entries.try_into().unwrap());
        let event_value = ScVal::Map(Some(outer_map));

        let result = parse_loan_from_rpc_event(&event_value);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("borrower_address not found"));
    }
}
