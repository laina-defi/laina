import 'dotenv/config';
import { mkdirSync } from 'fs';
import crypto from 'crypto';
import { CURRENCIES, type Currency } from '../currencies';
import {
  loadAccount,
  buildContracts,
  createContractBindings,
  createContractImports,
  exe,
  filenameNoExtension,
  installContracts,
  loanManagerAddress,
  readTextFile,
} from './util';

const account = process.env.SOROBAN_ACCOUNT;

console.log('######################Initializing contracts ########################');

const deploy = (wasm: string) => {
  exe(
    `stellar contract deploy --wasm ${wasm} --ignore-checks > ./.stellar/contract-ids/${filenameNoExtension(wasm)}.txt`,
  );
};

/** Deploy loan_manager contract as there will only be one for all the pools.
 * Loan_manager is used as a factory for the loan_pools.
 */
const deployLoanManager = () => {
  const contractsDir = `.stellar/contract-ids`;
  mkdirSync(contractsDir, { recursive: true });

  deploy(`./target/wasm32v1-none/release/loan_manager.wasm`);

  exe(`stellar contract invoke \
--id ${loanManagerAddress()} \
--source-account ${account} \
--network testnet \
-- initialize \
--admin ${account}`);
};

/** Deploy liquidity pools using the loan-manager as a factory contract */
const deployLoanPools = () => {
  const wasmHash = readTextFile('./.stellar/contract-wasm-hash/loan_pool.txt');

  CURRENCIES.forEach(({ tokenContractAddress, ticker, loanPoolName }: Currency) => {
    const salt = crypto.randomBytes(32).toString('hex');
    exe(
      `stellar contract invoke \
--id ${loanManagerAddress()} \
--source-account ${account} \
--network testnet \
-- deploy_pool \
--wasm_hash ${wasmHash} \
--salt ${salt} \
--token_address ${tokenContractAddress} \
--ticker ${ticker} \
--liquidation_threshold 8000000 \
| tr -d '"' > ./.stellar/contract-ids/${loanPoolName}.txt`,
    );
  });
};

// Calling the functions (equivalent to the last part of your bash script)
loadAccount();
buildContracts();
installContracts();
deployLoanManager();
deployLoanPools();
createContractBindings();
createContractImports();

console.log('\nInitialization successful!');
