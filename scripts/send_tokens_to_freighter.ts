import { config } from 'dotenv';

config({ path: '.env.local' });
import { Keypair, Horizon, TransactionBuilder, Operation, Asset, BASE_FEE } from '@stellar/stellar-sdk';

const horizonUrl = 'http://localhost:8000/';

const issuerKeypair = process.env.SOROBAN_SECRET_KEY
  ? Keypair.fromSecret(process.env.SOROBAN_SECRET_KEY)
  : Keypair.random();

console.log(`issuer keys:\n${issuerKeypair.publicKey()}\n${issuerKeypair.secret()}\n`);

const server = new Horizon.Server(horizonUrl, { allowHttp: true });
const eurcAsset = new Asset('EURC', issuerKeypair.publicKey());
const usdcAsset = new Asset('USDC', issuerKeypair.publicKey());

// Freighter2 wallet keypair from add_trustline.ts
const freighterKeypair = Keypair.fromSecret('SCJENYWNCT45S3DLKERA7MOFFUGRRWYSXKPJAFL6SCQ3I2SUDNRMCKCC');
console.log(`\n💰 Sending tokens to freighter: ${freighterKeypair.publicKey()}`);

// Load issuer account
const issuerAccount = await server.loadAccount(issuerKeypair.publicKey());

// Send 100k EURC and USDC to freighter2 wallet
const sendTransaction = new TransactionBuilder(issuerAccount, {
  fee: BASE_FEE,
  networkPassphrase: 'Standalone Network ; February 2017',
})
  .addOperation(
    Operation.payment({
      destination: freighterKeypair.publicKey(),
      asset: usdcAsset,
      amount: '100000.0000000', // Send 100k USDC
    }),
  )
  .addOperation(
    Operation.payment({
      destination: freighterKeypair.publicKey(),
      asset: eurcAsset,
      amount: '100000.0000000', // Send 100k EURC
    }),
  )
  .setTimeout(30)
  .build();

sendTransaction.sign(issuerKeypair);
const sendRes = await server.submitTransaction(sendTransaction);
console.log(`✅ Tokens sent: ${sendRes.hash}`);

console.log(`\n📋 Final Summary:`);
console.log(`Issuer: ${issuerKeypair.publicKey()}`);
console.log(`Freighter wallet: ${freighterKeypair.publicKey()}`);
console.log(`Sent 100,000 USDC and 100,000 EURC to freighter2 wallet`);
