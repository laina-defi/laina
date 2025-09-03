-- Your SQL goes here
CREATE TABLE loans (
  borrower_address TEXT NOT NULL,
  nonce BIGINT NOT NULL,
  borrowed_amount BIGINT NOT NULL,
  borrowed_from TEXT NOT NULL,
  collateral_amount BIGINT NOT NULL,
  collateral_from TEXT NOT NULL,
  unpaid_interest BIGINT NOT NULL,
  PRIMARY KEY (borrower_address, nonce)
);
