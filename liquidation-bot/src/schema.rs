// @generated automatically by Diesel CLI.

diesel::table! {
    loans (borrower_address, nonce) {
        borrower_address -> Text,
        nonce -> Int8,
        borrowed_amount -> Int8,
        borrowed_from -> Text,
        collateral_amount -> Int8,
        collateral_from -> Text,
        unpaid_interest -> Int8,
    }
}

diesel::table! {
    prices (id) {
        id -> Int4,
        pool_address -> Text,
        time_weighted_average_price -> Int8,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    loans,
    prices,
);
