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
  readTextFile,
  logDeploymentInfo,
  loanManagerAddress,
} from './util';
import { setPrice } from './set-oracle-price';

const account = process.env.SOROBAN_ACCOUNT;
const shouldDeployMockOracle = process.argv.includes('--mock-oracle');

let oracleAddressEnv = process.env.ORACLE_ADDRESS;

console.log('###################### Initializing contracts ########################');

const deploy = (wasm: string) => {
  exe(
    `stellar contract deploy --wasm ${wasm} --ignore-checks > ./.stellar/contract-ids/${filenameNoExtension(wasm)}.txt`,
  );
};

const deployMockOracle = (): string => {
  console.log('Deploying mock oracle (reflector_oracle_mock) ...');

  mkdirSync('./.stellar/contract-ids', { recursive: true });

  deploy(`./target/wasm32v1-none/release/reflector_oracle_mock.wasm`);
  const address = readTextFile('./.stellar/contract-ids/reflector_oracle_mock.txt');
  console.log(`Mock oracle deployed at: ${address}`);

  setPrice('XLM', '17694578912345', 'testnet', '1');
  setPrice('USDC', '17694578912345', 'testnet', '1');
  setPrice('EURC', '17694578912345', 'testnet', '1');

  return address;
};

/** Deploy loan_manager contract as there will only be one for all the pools.
 * Loan_manager is used as a factory for the loan_pools.
 */
const deployLoanManager = (oracleAddress: string) => {
  const contractsDir = `.stellar/contract-ids`;
  mkdirSync(contractsDir, { recursive: true });

  deploy(`./target/wasm32v1-none/release/loan_manager.wasm`);

  exe(`stellar contract invoke \
--id ${loanManagerAddress(true)} \
--source-account ${account} \
--network testnet \
-- initialize \
--admin ${account} \
--oracle_address ${oracleAddress}`);
};

/** Deploy liquidity pools using the loan-manager as a factory contract */
const deployLoanPools = () => {
  const wasmHash = readTextFile('./.stellar/contract-wasm-hash/loan_pool.txt');

  CURRENCIES.forEach(({ tokenContractAddress, ticker, loanPoolName }: Currency) => {
    const salt = crypto.randomBytes(32).toString('hex');
    exe(
      `stellar contract invoke \
--id ${loanManagerAddress(true)} \
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
installContracts(shouldDeployMockOracle);

// determine oracle address (deploy mock if requested)
const oracleForInit = shouldDeployMockOracle ? deployMockOracle() : (oracleAddressEnv as string);

deployLoanManager(oracleForInit);
deployLoanPools();
createContractBindings();
createContractImports();

console.log('\nInitialization successful!');
logDeploymentInfo();
