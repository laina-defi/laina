// @generated automatically by Diesel CLI.

diesel::table! {
    loans (id) {
        id -> Int4,
        borrowed_amount -> Int8,
        borrowed_from -> Text,
        borrower -> Text,
        collateral_amount -> Int8,
        collateral_from -> Text,
        unpaid_interest -> Int8,
    }
}

diesel::table! {
    prices (id) {
        id -> Int4,
        address -> Text,
        twap -> Int8,
    }
}

diesel::allow_tables_to_appear_in_same_query!(loans, prices,);
