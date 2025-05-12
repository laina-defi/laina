import { execSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

// Load environment variables starting with PUBLIC_ into the environment,
// so we don't need to specify duplicate variables in .env
for (const key in process.env) {
  if (key.startsWith('PUBLIC_')) {
    process.env[key.substring(7)] = process.env[key];
  }
}

// The stellar-sdk Client requires (for now) a defined public key. These are
// the Genesis accounts for each of the "typical" networks, and should work as
// a valid, funded network account.
const GENESIS_ACCOUNTS = {
  public: 'GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN7',
  testnet: 'GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H',
  futurenet: 'GADNDFP7HM3KFVHOQBBJDBGRONMKQVUYKXI6OYNDMS2ZIK7L6HA3F2RF',
  standalone: 'GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI',
};

// @ts-ignore
export const GENESIS_ACCOUNT = GENESIS_ACCOUNTS[process.env.SOROBAN_NETWORK] ?? GENESIS_ACCOUNTS.testnet;

export const loadAccount = () => {
  // This takes the secret key from SOROBAN_SECRET_KEY env-variable, so make sure you have that set.
  exe(`stellar keys add ${process.env.SOROBAN_ACCOUNT}`);
};

// Function to execute and log shell commands
export const exe = (command: string) => {
  console.log(command);
  return execSync(command, { stdio: 'inherit' });
};

export const buildContracts = () => {
  exe('rm -f ./target/wasm32-unknown-unknown/release/*.wasm');
  exe('rm -f ./target/wasm32-unknown-unknown/release/*.d');
  exe('make build');
};

/** Install all contracts and save their wasm hashes to .stellar */
export const installContracts = () => {
  const contractsDir = './.stellar/contract-wasm-hash';
  mkdirSync(contractsDir, { recursive: true });

  upload('loan_manager');
  upload('loan_pool');
};

/* Install a contract */
const upload = (contractName: string) => {
  exe(
    `stellar contract upload \
--wasm ./target/wasm32-unknown-unknown/release/${contractName}.wasm \
--ignore-checks \
> ./.stellar/contract-wasm-hash/${contractName}.txt`,
  );
};

export const filenameNoExtension = (filename: string) => {
  return path.basename(filename, path.extname(filename));
};

export const readTextFile = (path: string): string => readFileSync(path, { encoding: 'utf8' }).trim();

// This is a function so its value can update during init.
export const loanManagerAddress = (): string =>
  process.env.CONTRACT_ID_LOAN_MANAGER || readTextFile('./.stellar/contract-ids/loan_manager.txt');

export const createContractBindings = () => {
  bind('loan_manager', process.env.CONTRACT_ID_LOAN_MANAGER);
  bind('pool_xlm', process.env.CONTRACT_ID_POOL_XLM);
  bind('pool_usdc', process.env.CONTRACT_ID_POOL_USDC);
  bind('pool_eurc', process.env.CONTRACT_ID_POOL_EURC);
};

const bind = (contractName: string, address: string | undefined) => {
  const address_ = address || readTextFile(`./.stellar/contract-ids/${contractName}.txt`);
  exe(
    `stellar contract bindings typescript --contract-id ${address_} --output-dir ./packages/${contractName} --overwrite`,
  );
  exe(`cd ./packages/${contractName} && npm install && npm run build && cd ../..`);
};

export const createContractImports = () => {
  const CONTRACTS = ['loan_manager', 'pool_xlm', 'pool_usdc', 'pool_eurc'];
  CONTRACTS.forEach(importContract);
};

const importContract = (contractName: string) => {
  const outputDir = './src/contracts/';
  mkdirSync(outputDir, { recursive: true });

  /* eslint-disable quotes */
  /* eslint-disable no-constant-condition */
  const importContent =
    `import * as Client from '${contractName}'; \n` +
    `import { rpcUrl } from './util'; \n\n` +
    `export const contractId = Client.networks.${process.env.SOROBAN_NETWORK}.contractId; \n\n` +
    `export const contractClient = new Client.Client({ \n` +
    `  ...Client.networks.${process.env.SOROBAN_NETWORK}, \n` +
    `  rpcUrl, \n` +
    `${process.env.SOROBAN_NETWORK === 'local' || 'standalone' ? `  allowHttp: true,\n` : null}` +
    `  publicKey: '${GENESIS_ACCOUNT}', \n` +
    `}); \n`;

  const outputPath = `${outputDir}/${contractName}.ts`;
  writeFileSync(outputPath, importContent);
  console.log(`Created import for ${contractName}`);
};
