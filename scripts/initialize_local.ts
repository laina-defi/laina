import { config } from 'dotenv';

// Load local environment variables
config();
import { mkdirSync } from 'fs';
import crypto from 'crypto';
import { CURRENCIES_BY_TICKER } from '../currencies';
import {
  loadAccount,
  fundAccount,
  buildContracts,
  createContractBindings,
  createContractImports,
  exe,
  filenameNoExtension,
  installContracts,
  loanManagerAddress,
  readTextFile,
  writeContractIdsToEnv,
} from './util_local';

const account = process.env.SOROBAN_ACCOUNT;

console.log('######################Initializing contracts ########################');

const deploy = (wasm: string) => {
  exe(
    `stellar contract deploy --wasm ${wasm} --network local --ignore-checks > ./.stellar/contract-ids/${filenameNoExtension(wasm)}.txt`,
  );
};

/** Deploy loan_manager contract as there will only be one for all the pools.
 * Loan_manager is used as a factory for the loan_pools.
 */
const deployLoanManager = () => {
  const contractsDir = `.stellar/contract-ids`;
  mkdirSync(contractsDir, { recursive: true });

  deploy(`./target/wasm32v1-none/release/loan_manager.wasm`);

  // Read oracle address after it has been deployed
  const oracle = readTextFile('./.stellar/contract-ids/reflector_oracle_mock.txt');

  exe(`stellar contract invoke \
--id ${loanManagerAddress()} \
--source-account ${account} \
--network local \
-- initialize \
--admin ${account} \
--oracle_address ${oracle}`);
};

const deployShareToken = (name: string, symbol: string, fileBase: string): string => {
  exe(
    `stellar contract deploy \
--wasm ./target/wasm32v1-none/release/token.wasm \
--source-account ${account} \
--network local \
--ignore-checks \
-- \
--admin ${account} \
--decimal 7 \
--name "${name}" \
--symbol ${symbol} \
| tr -d '"' > ./.stellar/contract-ids/${fileBase}.txt`,
  );
  return readTextFile(`./.stellar/contract-ids/${fileBase}.txt`);
};

const deployInsurancePool = (poolAddress: string, shareTokenAddress: string, insurancePoolFile: string): string => {
  exe(
    `stellar contract deploy \
--wasm ./target/wasm32v1-none/release/insurance_pool.wasm \
--source-account ${account} \
--network local \
--ignore-checks \
| tr -d '"' > ./.stellar/contract-ids/${insurancePoolFile}.txt`,
  );
  const insurancePoolAddress = readTextFile(`./.stellar/contract-ids/${insurancePoolFile}.txt`);

  exe(
    `stellar contract invoke \
--id ${insurancePoolAddress} \
--source-account ${account} \
--network local \
-- initialize \
--loan_pool_addr ${poolAddress} \
--share_token_addr ${shareTokenAddress}`,
  );

  exe(
    `stellar contract invoke \
--id ${loanManagerAddress()} \
--source-account ${account} \
--network local \
-- set_insurance_pool \
--pool_addr ${poolAddress} \
--insurance_pool_addr ${insurancePoolAddress}`,
  );

  return insurancePoolAddress;
};

/** Deploy liquidity pools using the loan-manager as a factory contract */
const deployLoanPools = () => {
  const wasmHash = readTextFile('./.stellar/contract-wasm-hash/loan_pool.txt');

  const pools = [
    {
      tokenContractAddress: CURRENCIES_BY_TICKER.XLM.tokenContractAddress,
      ticker: 'XLM',
      loanPoolName: 'pool_xlm',
      shareTokenName: 'Laina XLM',
      shareTokenSymbol: 'lXLM',
      shareTokenFile: 'token_xlm',
      insurancePoolFile: 'insurance_pool_xlm',
    },
    {
      tokenContractAddress: CURRENCIES_BY_TICKER.USDC.tokenContractAddress,
      ticker: 'USDC',
      loanPoolName: 'pool_usdc',
      shareTokenName: 'Laina USDC',
      shareTokenSymbol: 'lUSDC',
      shareTokenFile: 'token_usdc',
      insurancePoolFile: 'insurance_pool_usdc',
    },
    {
      tokenContractAddress: CURRENCIES_BY_TICKER.EURC.tokenContractAddress,
      ticker: 'EURC',
      loanPoolName: 'pool_eurc',
      shareTokenName: 'Laina EURC',
      shareTokenSymbol: 'lEURC',
      shareTokenFile: 'token_eurc',
      insurancePoolFile: 'insurance_pool_eurc',
    },
  ];

  for (const {
    tokenContractAddress,
    ticker,
    loanPoolName,
    shareTokenName,
    shareTokenSymbol,
    shareTokenFile,
    insurancePoolFile,
  } of pools) {
    const shareTokenAddress = deployShareToken(shareTokenName, shareTokenSymbol, shareTokenFile);

    const salt = crypto.randomBytes(32).toString('hex');
    exe(
      `stellar contract invoke \
--id ${loanManagerAddress()} \
--source-account ${account} \
--network local \
-- deploy_pool \
--wasm_hash ${wasmHash} \
--salt ${salt} \
--token_address ${tokenContractAddress} \
--ticker ${ticker} \
--liquidation_threshold 8000000 \
--pool_token_address ${shareTokenAddress} \
| tr -d '"' > ./.stellar/contract-ids/${loanPoolName}.txt`,
    );

    const poolAddress = readTextFile(`./.stellar/contract-ids/${loanPoolName}.txt`);
    exe(
      `stellar contract invoke \
--id ${shareTokenAddress} \
--source-account ${account} \
--network local \
-- set_admin \
--new_admin ${poolAddress}`,
    );

    deployInsurancePool(poolAddress, shareTokenAddress, insurancePoolFile);
  }
};

/** Deploy reflector_mock contract */
const deployReflectorMock = () => {
  const contractsDir = `.stellar/contract-ids`;
  mkdirSync(contractsDir, { recursive: true });

  deploy(`./target/wasm32v1-none/release/reflector_oracle_mock.wasm`);
};

const deployNativeStellarAssetContract = () => {
  exe(`stellar contract asset deploy --asset native --network local --source-account ${account}`);
};

// Calling the functions (equivalent to the last part of your bash script)
loadAccount();
fundAccount();
buildContracts();
installContracts();
deployReflectorMock();
deployLoanManager();
deployLoanPools();
createContractBindings();
createContractImports();
writeContractIdsToEnv();
deployNativeStellarAssetContract();

console.log('\nInitialization successful!');
