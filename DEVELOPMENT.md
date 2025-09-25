# Development (Testnet)

This document describes how to develop the application on your local computer.

## Prerequirements

Tools you will need:

- [Rust & Cargo](https://www.rust-lang.org/learn/get-started)
- [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli)
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

Or if you want to use a mock oracle to allow changing the price of a token run:

```
npm run init:mock-oracle
```

You'll have to grab that oracle's address and place it in .env for liquidation bot to know to use it.

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
cp .env.local.example .env
```

## Setting Up an Account for Local Network

Simply create an alias with

```sh
stellar keys generate ci_local
```

Then print out your new public key:

```sh
stellar keys address ci_local
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

You can simply run the following command:

```sh
npm run init:local
```

This will deploy all of the contracts, generate TypeScript bindings, issue and create sell offers for two custom tokens USDC and EURC, set token prices, deploy XLM SAC, etc.

## Frontend

After the init script has finished running, you can start the frontend server by running:

```sh
npm run dev:local
```
