import { config } from 'dotenv';

config({ path: '.env.local' });
import { Keypair, Horizon, TransactionBuilder, Operation, Asset, BASE_FEE } from '@stellar/stellar-sdk';

const horizonUrl = 'http://localhost:8000/';

const issuerKeypair = process.env.SOROBAN_SECRET_KEY
  ? Keypair.fromSecret(process.env.SOROBAN_SECRET_KEY)
  : Keypair.random();

const server = new Horizon.Server(horizonUrl, { allowHttp: true });
const eurcAsset = new Asset('EURC', issuerKeypair.publicKey());
const usdcAsset = new Asset('USDC', issuerKeypair.publicKey());

// Create a random recipient account and send tokens to make them visible in wallets
const recipientKeypair = Keypair.fromSecret('SCJENYWNCT45S3DLKERA7MOFFUGRRWYSXKPJAFL6SCQ3I2SUDNRMCKCC');
console.log(`\n👤 Creating recipient account: ${recipientKeypair.publicKey()}`);

const recipientAccount = await server.loadAccount(recipientKeypair.publicKey());

// Create trustlines and send tokens
const trustlineTransaction = new TransactionBuilder(recipientAccount, {
  fee: BASE_FEE,
  networkPassphrase: 'Standalone Network ; February 2017',
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
