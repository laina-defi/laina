import 'dotenv/config';
import { mkdirSync } from 'fs';
import crypto from 'crypto';
import { execSync } from 'child_process';
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
  writeEnvVar,
} from './util';
import { Keypair } from '@stellar/stellar-sdk';
import { setPrice } from './set-oracle-price';
import { issueTokens, type IssuedTokens } from './issue_tokens';
import { seedPools, fundFaucet } from './seed_pools';

const HORIZON_URL = 'https://horizon-testnet.stellar.org';

const account = process.env.SOROBAN_ACCOUNT;
const shouldDeployMockOracle = process.argv.includes('--mock-oracle');

console.log('###################### Initializing contracts ########################');

const deploy = (wasm: string) => {
  exe(
    `stellar contract deploy --wasm ${wasm} --ignore-checks > ./.stellar/contract-ids/${filenameNoExtension(wasm)}.txt`,
  );
};

const deployMockOracle = (): string => {
  mkdirSync('./.stellar/contract-ids', { recursive: true });
  deploy(`./target/wasm32v1-none/release/reflector_oracle_mock.wasm`);
  const address = readTextFile('./.stellar/contract-ids/reflector_oracle_mock.txt');
  setPrice('XLM', '17694578912345', 'testnet', '1');
  setPrice('USDC', '17694578912345', 'testnet', '1');
  setPrice('EURC', '17694578912345', 'testnet', '1');
  return address;
};

const deployLoanManager = (oracleAddress: string) => {
  mkdirSync(`.stellar/contract-ids`, { recursive: true });
  deploy(`./target/wasm32v1-none/release/loan_manager.wasm`);
  exe(`stellar contract invoke \
--id ${loanManagerAddress(true)} \
--source-account ${account} \
--network testnet \
-- initialize \
--admin ${account} \
--oracle_address ${oracleAddress}`);
};

const deployNativeXlmSac = (): string => {
  let address: string;
  try {
    address = execSync(
      `stellar contract asset deploy --asset native --network testnet --source-account ${account}`,
      { encoding: 'utf-8', stdio: ['pipe', 'pipe', 'pipe'] },
    ).trim();
    console.log(`Native XLM SAC deployed: ${address}`);
  } catch {
    address = execSync(
      `stellar contract id asset --asset native --network testnet`,
      { encoding: 'utf-8' },
    ).trim();
    console.log(`Native XLM SAC already exists: ${address}`);
  }
  writeEnvVar('PUBLIC_CONTRACT_ADDRESS_XLM', address);
  return address;
};

const deployShareToken = (name: string, symbol: string, fileBase: string): string => {
  exe(
    `stellar contract deploy \
--wasm ./target/wasm32v1-none/release/token.wasm \
--source-account ${account} \
--network testnet \
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
--network testnet \
--ignore-checks \
| tr -d '"' > ./.stellar/contract-ids/${insurancePoolFile}.txt`,
  );
  const insurancePoolAddress = readTextFile(`./.stellar/contract-ids/${insurancePoolFile}.txt`);

  exe(
    `stellar contract invoke \
--id ${insurancePoolAddress} \
--source-account ${account} \
--network testnet \
-- initialize \
--loan_pool_addr ${poolAddress} \
--share_token_addr ${shareTokenAddress} \
--loan_manager_addr ${loanManagerAddress(true)}`,
  );

  exe(
    `stellar contract invoke \
--id ${loanManagerAddress(true)} \
--source-account ${account} \
--network testnet \
-- set_insurance_pool \
--pool_addr ${poolAddress} \
--insurance_pool_addr ${insurancePoolAddress}`,
  );

  return insurancePoolAddress;
};

const deployLoanPools = (tokens: IssuedTokens, xlmAddress: string) => {
  const wasmHash = readTextFile('./.stellar/contract-wasm-hash/loan_pool.txt');

  const pools = [
    { tokenAddress: xlmAddress,         ticker: 'XLM',  poolName: 'pool_xlm',  shareTokenName: 'Laina XLM',  shareTokenSymbol: 'lXLM',  shareTokenFile: 'token_xlm',  insurancePoolFile: 'insurance_pool_xlm'  },
    { tokenAddress: tokens.usdcAddress, ticker: 'USDC', poolName: 'pool_usdc', shareTokenName: 'Laina USDC', shareTokenSymbol: 'lUSDC', shareTokenFile: 'token_usdc', insurancePoolFile: 'insurance_pool_usdc' },
    { tokenAddress: tokens.eurcAddress, ticker: 'EURC', poolName: 'pool_eurc', shareTokenName: 'Laina EURC', shareTokenSymbol: 'lEURC', shareTokenFile: 'token_eurc', insurancePoolFile: 'insurance_pool_eurc' },
  ];

  for (const { tokenAddress, ticker, poolName, shareTokenName, shareTokenSymbol, shareTokenFile, insurancePoolFile } of pools) {
    const shareTokenAddress = deployShareToken(shareTokenName, shareTokenSymbol, shareTokenFile);

    const salt = crypto.randomBytes(32).toString('hex');
    exe(
      `stellar contract invoke \
--id ${loanManagerAddress(true)} \
--source-account ${account} \
--network testnet \
-- deploy_pool \
--wasm_hash ${wasmHash} \
--salt ${salt} \
--token_address ${tokenAddress} \
--ticker ${ticker} \
--liquidation_threshold 8000000 \
--pool_token_address ${shareTokenAddress} \
| tr -d '"' > ./.stellar/contract-ids/${poolName}.txt`,
    );

    const poolAddress = readTextFile(`./.stellar/contract-ids/${poolName}.txt`);
    exe(
      `stellar contract invoke \
--id ${shareTokenAddress} \
--source-account ${account} \
--network testnet \
-- set_admin \
--new_admin ${poolAddress}`,
    );

    deployInsurancePool(poolAddress, shareTokenAddress, insurancePoolFile);
  }
};

const initializeLaiDistribution = async (tokens: IssuedTokens) => {
  const loanManager = loanManagerAddress(true);
  const SCALAR_7 = 10_000_000n;
  const amount = 50_000_000n * SCALAR_7;

  // Transfer 50M LAI from issuer to loan_manager via SAC (contracts can't receive classic payments)
  exe(
    `stellar contract invoke \
--id ${tokens.laiAddress} \
--source-account ${tokens.issuer.secret()} \
--network testnet \
-- transfer \
--from ${tokens.issuer.publicKey()} \
--to ${loanManager} \
--amount ${amount}`,
  );
  console.log(`Transferred 50M LAI to loan_manager (${loanManager})`);

  // Get current ledger for start_ledger
  const ledgerRes = await fetch(`${HORIZON_URL}/`);
  const ledgerJson = await ledgerRes.json() as { core_latest_ledger: number };
  const currentLedger = ledgerJson.core_latest_ledger;

  exe(
    `stellar contract invoke \
--id ${loanManager} \
--source-account ${account} \
--network testnet \
-- initialize_lai_distribution \
--token ${tokens.laiAddress} \
--start_ledger ${currentLedger}`,
  );
  console.log(`LAI distribution initialized at ledger ${currentLedger}`);
};

const deployFaucet = (tokens: IssuedTokens): string => {
  deploy(`./target/wasm32v1-none/release/faucet.wasm`);
  const faucetAddress = readTextFile('./.stellar/contract-ids/faucet.txt');

  exe(`stellar contract invoke \
--id ${faucetAddress} \
--source-account ${account} \
--network testnet \
-- initialize \
--usdc ${tokens.usdcAddress} \
--eurc ${tokens.eurcAddress} \
--lai ${tokens.laiAddress}`);

  writeEnvVar('PUBLIC_CONTRACT_ADDRESS_FAUCET', faucetAddress);
  return faucetAddress;
};

// Fund the deployer account via friendbot before anything else.
const deployerPublicKey = Keypair.fromSecret(process.env.SOROBAN_SECRET_KEY!).publicKey();
const fundRes = await fetch(`https://friendbot.stellar.org?addr=${deployerPublicKey}`);
if (!fundRes.ok && fundRes.status !== 400) {
  throw new Error(`Friendbot failed: ${fundRes.statusText}`);
}
console.log(`Deployer account funded (${deployerPublicKey})`);

// Issue tokens first so pool deployment uses the correct SAC addresses.
const tokens = await issueTokens();

loadAccount();
buildContracts();
installContracts(shouldDeployMockOracle);

const oracleForInit = shouldDeployMockOracle ? deployMockOracle() : (process.env.ORACLE_ADDRESS as string);

deployLoanManager(oracleForInit);
const xlmAddress = deployNativeXlmSac();
deployLoanPools(tokens, xlmAddress);
await initializeLaiDistribution(tokens);

const xlmPoolId = readTextFile('./.stellar/contract-ids/pool_xlm.txt');
const usdcPoolId = readTextFile('./.stellar/contract-ids/pool_usdc.txt');
const eurcPoolId = readTextFile('./.stellar/contract-ids/pool_eurc.txt');

await seedPools(tokens, account!, xlmPoolId, usdcPoolId, eurcPoolId);

const faucetAddress = deployFaucet(tokens);
await fundFaucet(tokens, faucetAddress);

// Set env var so contract bindings pick up the faucet address
process.env.CONTRACT_ID_FAUCET = faucetAddress;

createContractBindings();
createContractImports();

console.log('\nInitialization successful!');
logDeploymentInfo();
process.exit(0);
