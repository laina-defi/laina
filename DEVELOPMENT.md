# Development (Testnet)

This document describes how to develop the application on your local computer.

## Prerequirements

Tools you will need:

- [Rust & Cargo](https://www.rust-lang.org/learn/get-started)
- [Node.js - check version from .nvmrc](https://nodejs.org/en)
- [Docker & docker-compose](https://docs.docker.com/compose/install/)
- [Diesel CLI](https://diesel.rs/guides/getting-started#installing-diesel-cli)

## Rust Smart Contracts

Build the contracts. This also compiles the liquidation bot.
Look in `Makefile` for commands for compiling a specific binary.

```
make Build
```

To deploy new smart contracts to testnet, run the `scripts/initialize.ts` script:

```
npm run init
```

To update the code of already initialized contracts in-place, use the `scripts/upgrade.ts` script.

```
npm run upgrade
```

Run tests

```
cargo test
```

Format code

```
cargo fmt
```

## TypeScript & React DApp

Start the development server

```
npm run dev
```

Run linter

```
npm run lint
```

Run formatter

```
npm run format
```

## Rust Liquidation Bot

Start the development database. You can use the `-d` flag if you want to detach your terminal from the output.

```
docker-compose up
```

Run database migrations

```
cd liquidation-bot
diesel migration run
```

Start liquidation-bot

```
cargo run
```

# Development (Local network)

Development in the local network gives some added benefits compared to testnet. In this case, much faster ledger times and possibility to modify time and in addition mock contracts without loading testnet.

## Prerequirements

Same as in Testnet.

## Environment

Copy the example:

```sh
cp .local.env.example .env.local
```

## Setting Up the Local Network

We are using a ephemeral mode, which means that every time you start the container the database starts empty. Due to this, there's a bit more to setting up to do. First, you should start the container:

```sh
docker compose up
```

This will start both the liquidation bots database and the whole local Stellar network including Horizon, RPC, and Friendbot. After the network has started and synced you need to create an account. If you don't have existing stellar id's you can run:

```sh
stellar keys generate ci_local --network local
```

Then print out your new public key:

```sh
stellar keys address ci_local
```

And then fund the account using the local friendbot. Note, you need to change your public key to the url. If you had existing account alias you could skip generating a new one.

```sh
curl "http://localhost:8000/friendbot?addr=GAMX6...J3I2U"
```

Finally, print out your secret key:

```sh
stellar keys show ci_local
```

And add it to `.env.local`

```sh
SOROBAN_SECRET_KEY="S...SZG"
```

## Deploying Contracts

First we need to build the contracts.

```sh
make build
```

Then we need to deploy a mock price oracle as we need it's address for the loan manager.

```sh
stellar contract deploy --wasm ./target/wasm32v1-none/release/reflector_oracle_mock.wasm --network local --source-account ci_local --ignore-checks --salt 1
```

Using `--salt 1` is optional, but having constant salt removes the need to change the oracle address on every restart of the local network. After this you should change the returned contract ID to `laina/contracts/loan_manager/src/contract.rs`

```rust
const REFLECTOR_ADDRESS: &str = "CBLPQLLG3I2VN3MSULBVHYJNQNPMDHF44RW5I24VZUI3NRAKI7PZ3ZYM";
```

Then add the same address to `.env.local`:

```sh
ORACLE_ADDRESS="CAIZU6FXBMNR4O2ZBS6P2Z7JZMUIHM6MK2GL7TLAOBEIM4KKEONT3LAE"
```

Now you can simply run the initialization script:

```sh
npm run init:local
```

This will deploy all of the contracts, generate TypeScript bindings, and issue and create sell offers for two custom tokens USDC and EURC. When the script has ran it will show you something like this:

```sh
🚀 Deploying Stellar Asset Contracts...
ℹ️ Signing transaction: a513751201b712e9c80df2b1cdbad08e8b8a4b87fc92349192bb4a570fcbb0f5
✅ USDC Contract Address: CDU4BRSZXYHAN3XINOJEKGUQ4WNLSLVQC5H6F6N75ORBU5YMNTPIJH7H
ℹ️ Signing transaction: a5dfeea136bf3e75b673f9fc270131730b59b828431a5881f02ea78dc7562cfe
✅ EURC Contract Address: CBA4EB6OXQOP3VOT36P7JX3ALWMRNGVEDIILIY42H5K7SKJTNXXM24FN

📋 Asset Summary:
USDC Issuer: GAMX6CTD62UMM7EH24ULHZZWN3K3WI6BVHGQZE5HOCZSRBDNKT2J3I2U
USDC Contract: CDU4BRSZXYHAN3XINOJEKGUQ4WNLSLVQC5H6F6N75ORBU5YMNTPIJH7H
EURC Issuer: GAMX6CTD62UMM7EH24ULHZZWN3K3WI6BVHGQZE5HOCZSRBDNKT2J3I2U
EURC Contract: CBA4EB6OXQOP3VOT36P7JX3ALWMRNGVEDIILIY42H5K7SKJTNXXM24FN
```

Finally, we need to deploy a SAC (Stellar Asset Contract) for the native token, XLM:

```sh
stellar contract asset deploy --asset native --network local --source-account ci_local
```

You can use this addresses from asset summary and SAC deployment to setup `currencies_local.ts` to use these local network custom tokens.

## Frontend

After you have updated `currencies_local.ts` to use the abovementioned addresses, you can simply run:

```sh
npm run dev:local
```
