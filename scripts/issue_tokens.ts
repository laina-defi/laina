import { config } from 'dotenv';

config({ path: '.env.local' });
import { Keypair, Horizon, TransactionBuilder, Networks, Operation, Asset, BASE_FEE } from '@stellar/stellar-sdk';

const horizonUrl = 'https://horizon-testnet.stellar.org';

const issuerKeypair = Keypair.random();

console.log(`issuer keys:\n${issuerKeypair.publicKey()}\n${issuerKeypair.secret()}\n`);

// Fund the account using friendbot
const friendbotUrl = 'https://friendbot.stellar.org';
try {
  const response = await fetch(friendbotUrl + `?addr=${issuerKeypair.publicKey()}`);
  if (response.ok) {
    console.log(`✅ Funded account: ${issuerKeypair.publicKey()}`);
  } else {
    console.log(`❌ Failed to fund account: ${issuerKeypair.publicKey()}`);
  }
} catch (error) {
  console.log(`❌ Error funding account: ${error}`);
}

const server = new Horizon.Server(horizonUrl);
const account = await server.loadAccount(issuerKeypair.publicKey());
const eurcAsset = new Asset('EURC', issuerKeypair.publicKey());
const usdcAsset = new Asset('USDC', issuerKeypair.publicKey());

const transaction = new TransactionBuilder(account, {
  fee: BASE_FEE,
  networkPassphrase: Networks.TESTNET,
})
  .addOperation(
    Operation.payment({
      destination: issuerKeypair.publicKey(),
      asset: eurcAsset,
      amount: '1000000000', // Mint EURC to yourself
    }),
  )
  .addOperation(
    Operation.payment({
      destination: issuerKeypair.publicKey(),
      asset: usdcAsset,
      amount: '1000000000', // Mint USDC to yourself
    }),
  )
  .addOperation(
    Operation.createPassiveSellOffer({
      selling: eurcAsset,
      buying: Asset.native(),
      amount: '100000000', // Sell 10% of minted EURC
      price: 0.1,
    }),
  )
  .addOperation(
    Operation.createPassiveSellOffer({
      selling: usdcAsset,
      buying: Asset.native(),
      amount: '100000000', // Sell 10% of minted USDC
      price: 0.1,
    }),
  )
  .setTimeout(30)
  .build();

transaction.sign(issuerKeypair);
const res = await server.submitTransaction(transaction);
console.log(`✅ Transaction hash: ${res.hash}`);

// Create a random recipient account and send tokens to make them visible in wallets
const recipientKeypair = Keypair.random();
console.log(`\n👤 Creating recipient account: ${recipientKeypair.publicKey()}`);

// Fund recipient account
try {
  const response = await fetch(friendbotUrl + `?addr=${recipientKeypair.publicKey()}`);
  if (response.ok) {
    console.log(`✅ Funded recipient account`);
  }
} catch (error) {
  console.log(`❌ Error funding recipient: ${error}`);
}

// Wait a moment for the account to be created
await new Promise((resolve) => setTimeout(resolve, 2000));

// Load accounts
const issuerAccount = await server.loadAccount(issuerKeypair.publicKey());
const recipientAccount = await server.loadAccount(recipientKeypair.publicKey());

// Create trustlines and send tokens
const trustlineTransaction = new TransactionBuilder(recipientAccount, {
  fee: BASE_FEE,
  networkPassphrase: Networks.TESTNET,
})
  .addOperation(
    Operation.changeTrust({
      asset: usdcAsset,
      source: recipientKeypair.publicKey(),
    }),
  )
  .addOperation(
    Operation.changeTrust({
      asset: eurcAsset,
      source: recipientKeypair.publicKey(),
    }),
  )
  .setTimeout(30)
  .build();

trustlineTransaction.sign(recipientKeypair);
const trustRes = await server.submitTransaction(trustlineTransaction);
console.log(`✅ Trustlines created: ${trustRes.hash}`);

// Send tokens to recipient
const sendTransaction = new TransactionBuilder(issuerAccount, {
  fee: BASE_FEE,
  networkPassphrase: Networks.TESTNET,
})
  .addOperation(
    Operation.payment({
      destination: recipientKeypair.publicKey(),
      asset: usdcAsset,
      amount: '10.0000000', // Send 10 USDC
    }),
  )
  .addOperation(
    Operation.payment({
      destination: recipientKeypair.publicKey(),
      asset: eurcAsset,
      amount: '10.0000000', // Send 10 EURC
    }),
  )
  .setTimeout(30)
  .build();

sendTransaction.sign(issuerKeypair);
const sendRes = await server.submitTransaction(sendTransaction);
console.log(`✅ Tokens sent: ${sendRes.hash}`);

console.log(`\n📋 Final Summary:`);
console.log(`Issuer: ${issuerKeypair.publicKey()}`);
console.log(`Recipient: ${recipientKeypair.publicKey()}`);
console.log(`Recipient received 10 USDC and 10 EURC`);

// Deploy Stellar Asset Contracts for the custom tokens
import { execSync } from 'child_process';

console.log('\n🚀 Deploying Stellar Asset Contracts...');

try {
  // Deploy USDC asset contract
  const usdcContractAddress = execSync(
    `stellar contract asset deploy --asset USDC:${issuerKeypair.publicKey()} --network testnet --source-account ${issuerKeypair.secret()}`,
    { encoding: 'utf-8' },
  ).trim();

  console.log(`✅ USDC Contract Address: ${usdcContractAddress}`);

  // Deploy EURC asset contract
  const eurcContractAddress = execSync(
    `stellar contract asset deploy --asset EURC:${issuerKeypair.publicKey()} --network testnet --source-account ${issuerKeypair.secret()}`,
    { encoding: 'utf-8' },
  ).trim();

  console.log(`✅ EURC Contract Address: ${eurcContractAddress}`);

  console.log('\n📋 Asset Summary:');
  console.log(`USDC Issuer: ${issuerKeypair.publicKey()}`);
  console.log(`USDC Contract: ${usdcContractAddress}`);
  console.log(`EURC Issuer: ${issuerKeypair.publicKey()}`);
  console.log(`EURC Contract: ${eurcContractAddress}`);
} catch (error) {
  console.error('❌ Error deploying asset contracts:', error instanceof Error ? error.message : String(error));
}
