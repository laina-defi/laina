import { execSync } from 'child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'fs';
import path from 'path';

/** Update or append a key=value pair in .env */
export const writeEnvVar = (key: string, value: string, envPath = '.env') => {
  let content = '';
  try {
    content = readFileSync(envPath, 'utf8');
  } catch {
    /* file may not exist yet */
  }
  const line = `${key}=${value}`;
  const re = new RegExp(`^${key}=.*`, 'm');
  content = re.test(content) ? content.replace(re, line) : `${content}\n${line}`;
  writeFileSync(envPath, content.trimStart());
};

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
const network = process.env.SOROBAN_NETWORK;
export const GENESIS_ACCOUNT = GENESIS_ACCOUNTS[network as keyof typeof GENESIS_ACCOUNTS] ?? GENESIS_ACCOUNTS.testnet;

export const loadAccount = () => {
  // Remove first so re-runs don't fail on "key already exists".
  exe(`stellar keys rm ${process.env.SOROBAN_ACCOUNT} 2>/dev/null || true`);
  exe(`stellar keys add ${process.env.SOROBAN_ACCOUNT}`);
};

// Function to execute and log shell commands
export const exe = (command: string) => {
  console.log(command);
  return execSync(command, { stdio: 'inherit' });
};

export const buildContracts = () => {
  exe(`rm -f ./target/wasm32v1-none/release/*.wasm`);
  exe(`rm -f ./target/wasm32v1-none/release/*.d`);
  exe(`make build`);
};

/** Install all contracts and save their wasm hashes to .stellar */
export const installContracts = (mockOracle: boolean = false) => {
  const contractsDir = `./.stellar/contract-wasm-hash`;
  mkdirSync(contractsDir, { recursive: true });

  install('loan_manager');
  install('loan_pool');
  install('insurance_pool');
  install('token');
  install('faucet');
  if (mockOracle) {
    install('reflector_oracle_mock');
  }
};

/* Install a contract */
const install = (contractName: string) => {
  exe(
    `stellar contract upload \
--wasm ./target/wasm32v1-none/release/${contractName}.wasm \
--ignore-checks \
> ./.stellar/contract-wasm-hash/${contractName}.txt`,
  );
};

export const filenameNoExtension = (filename: string) => {
  return path.basename(filename, path.extname(filename));
};

export const readTextFile = (path: string): string => readFileSync(path, { encoding: 'utf8' }).trim();

// This is a function so its value can update during init.
export const loanManagerAddress = (init = false): string => {
  if (init) {
    // Ignore the env variable if we want to initialize a new manager.
    return readTextFile('./.stellar/contract-ids/loan_manager.txt');
  }

  return process.env.CONTRACT_ID_LOAN_MANAGER || readTextFile('./.stellar/contract-ids/loan_manager.txt');
};

export const createContractBindings = () => {
  bind('loan_manager', process.env.CONTRACT_ID_LOAN_MANAGER);
  bind('pool_xlm', process.env.CONTRACT_ID_POOL_XLM);
  bind('pool_usdc', process.env.CONTRACT_ID_POOL_USDC);
  bind('pool_eurc', process.env.CONTRACT_ID_POOL_EURC);
  bind('faucet', process.env.CONTRACT_ID_FAUCET);
  bind('insurance_pool_xlm', process.env.CONTRACT_ID_INSURANCE_POOL_XLM);
  bind('insurance_pool_eurc', process.env.CONTRACT_ID_INSURANCE_POOL_EURC);
  bind('insurance_pool_usdc', process.env.CONTRACT_ID_INSURANCE_POOL_USDC);
};

const bind = (contractName: string, address: string | undefined) => {
  const address_ = address || readTextFile(`./.stellar/contract-ids/${contractName}.txt`);
  exe(
    `stellar contract bindings typescript --contract-id ${address_} --output-dir ./packages/${contractName} --overwrite`,
  );
  exe(`cd ./packages/${contractName} && npm install && npm run build && cd ../..`);
};

export const createContractImports = () => {
  const CONTRACTS = [
    'loan_manager',
    'pool_xlm',
    'pool_usdc',
    'pool_eurc',
    'faucet',
    'insurance_pool_xlm',
    'insurance_pool_usdc',
    'insurance_pool_eurc',
  ];
  CONTRACTS.forEach(importContract);
};

const importContract = (contractName: string) => {
  const outputDir = `./src/contracts/`;
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

export const withRetry = async <T>(fn: () => Promise<T>, retries = 4, baseDelayMs = 3000): Promise<T> => {
  for (let attempt = 1; attempt <= retries; attempt++) {
    try {
      return await fn();
    } catch (err: unknown) {
      if (attempt === retries) throw err;
      const msg = err instanceof Error ? err.message : String(err);
      console.log(`Horizon error (attempt ${attempt}/${retries}): ${msg} — retrying in ${baseDelayMs * attempt}ms`);
      await new Promise((r) => setTimeout(r, baseDelayMs * attempt));
    }
  }
  throw new Error('unreachable');
};

export const logDeploymentInfo = () => {
  console.log(`  Network: ${process.env.SOROBAN_NETWORK}`);
  console.log(`  Account: ${process.env.SOROBAN_ACCOUNT}`);
  console.log(
    `  Loan Manager address: ${process.env.CONTRACT_ID_LOAN_MANAGER || readTextFile('./.stellar/contract-ids/loan_manager.txt')}`,
  );
  console.log(
    `  XLM Pool address: ${process.env.CONTRACT_ID_POOL_XLM || readTextFile('./.stellar/contract-ids/pool_xlm.txt')}`,
  );
  console.log(
    `  USDC Pool address: ${process.env.CONTRACT_ID_POOL_USDC || readTextFile('./.stellar/contract-ids/pool_usdc.txt')}`,
  );
  console.log(
    `  EURC Pool address: ${process.env.CONTRACT_ID_POOL_EURC || readTextFile('./.stellar/contract-ids/pool_eurc.txt')}`,
  );
};
