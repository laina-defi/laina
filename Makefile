build:
	mkdir -p target/wasm32v1-none/release
	curl -L https://github.com/reflector-network/reflector-contract/releases/download/v4.1.0_reflector-oracle_v4.1.0.wasm/reflector-oracle_v4.1.0.wasm -o ./target/wasm32v1-none/release/reflector_oracle.wasm
	cargo build --release --target wasm32v1-none -p reflector-oracle-mock
	cargo build --release --target wasm32v1-none -p loan_pool
	cargo build --release --target wasm32v1-none -p loan_manager
	cargo build --release -p liquidation-bot
