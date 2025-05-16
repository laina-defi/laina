use soroban_sdk::contractimport;

#[cfg(not(test))]
contractimport!(file = "../../target/wasm32v1-none/release/reflector_oracle.wasm");

#[cfg(test)]
contractimport!(file = "../../target/wasm32v1-none/release/reflector_oracle_mock.wasm");
