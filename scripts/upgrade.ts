import 'dotenv/config';
import {
  buildContracts,
  createContractBindings,
  createContractImports,
  exe,
  installContracts,
  loadAccount,
  loanManagerAddress,
  logDeploymentInfo,
  readTextFile,
} from './util';

console.log('######################Updating contracts ########################');

// Invoke the upgrade-action of loan manager. It will upgrade its pools as well.
const upgradeContracts = () => {
  const managerWasmHash = readTextFile('./.stellar/contract-wasm-hash/loan_manager.txt');
  const poolWasmHash = readTextFile('./.stellar/contract-wasm-hash/loan_pool.txt');

  exe(`stellar contract invoke \
--id ${loanManagerAddress()} \
--source-account ${process.env.SOROBAN_ACCOUNT} \
--network testnet \
-- \
upgrade \
--new_manager_wasm_hash ${managerWasmHash} \
--new_pool_wasm_hash ${poolWasmHash}`);
};

loadAccount();
buildContracts();
installContracts();
upgradeContracts();
createContractBindings();
createContractImports();

console.log('\nUpgrade successful!');
logDeploymentInfo();
