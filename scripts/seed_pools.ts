/**
 * Seeds each lending pool with a random amount between 9,000–11,000 tokens
 * and funds the faucet contract with 500,000 of each token.
 *
 * Must be called after pools and faucet are deployed.
 */
import { Keypair, Horizon, TransactionBuilder, Networks, Operation, Asset, BASE_FEE } from '@stellar/stellar-sdk';
import { exe, withRetry } from './util';
import type { IssuedTokens } from './issue_tokens';

const HORIZON_URL = 'https://horizon-testnet.stellar.org';
const NETWORK = Networks.TESTNET;

const server = new Horizon.Server(HORIZON_URL);
const SCALAR_7 = 10_000_000n;

// XLM comes from the deployer's own balance (capped by friendbot at 10k).
// USDC/EURC are minted freely from the issuer, so a larger seed is fine.
const randomXlmAmount = () => BigInt(500 + Math.floor(Math.random() * 501)) * SCALAR_7;  // 500–1000 XLM
const randomStablecoinAmount = () => BigInt(9000 + Math.floor(Math.random() * 2001)) * SCALAR_7; // 9000–11000

const setupTrustlineAndFund = async (
  recipient: Keypair,
  issuer: Keypair,
  tickers: string[],
  amounts: string[],
) => {
  const recipientAccount = await withRetry(() => server.loadAccount(recipient.publicKey()));
  const issuerAccount = await withRetry(() => server.loadAccount(issuer.publicKey()));

  const trustTx = new TransactionBuilder(recipientAccount, { fee: BASE_FEE, networkPassphrase: NETWORK })
    .setTimeout(30);
  for (const ticker of tickers) {
    trustTx.addOperation(Operation.changeTrust({ asset: new Asset(ticker, issuer.publicKey()) }));
  }
  const builtTrustTx = trustTx.build();
  builtTrustTx.sign(recipient);
  await withRetry(() => server.submitTransaction(builtTrustTx));

  const sendTx = new TransactionBuilder(issuerAccount, { fee: BASE_FEE, networkPassphrase: NETWORK })
    .setTimeout(30);
  for (let i = 0; i < tickers.length; i++) {
    sendTx.addOperation(
      Operation.payment({ destination: recipient.publicKey(), asset: new Asset(tickers[i]!, issuer.publicKey()), amount: amounts[i]! }),
    );
  }
  const builtSendTx = sendTx.build();
  builtSendTx.sign(issuer);
  await withRetry(() => server.submitTransaction(builtSendTx));
};

export const seedPools = async (
  tokens: IssuedTokens,
  account: string,
  xlmPoolId: string,
  usdcPoolId: string,
  eurcPoolId: string,
) => {
  const { issuer } = tokens;
  const secretKey = process.env.SOROBAN_SECRET_KEY;
  if (!secretKey) throw new Error('SOROBAN_SECRET_KEY not set');
  const initKeypair = Keypair.fromSecret(secretKey);

  const xlmAmount = randomXlmAmount();
  const usdcAmount = randomStablecoinAmount();
  const eurcAmount = randomStablecoinAmount();

  // Give init account trustlines + tokens for pool seeding
  await setupTrustlineAndFund(
    initKeypair,
    issuer,
    ['USDC', 'EURC'],
    [(usdcAmount / SCALAR_7).toString(), (eurcAmount / SCALAR_7).toString()],
  );

  console.log(`Seeding XLM pool with ${xlmAmount / SCALAR_7} XLM...`);
  exe(`stellar contract invoke --id ${xlmPoolId} --source-account ${account} --network testnet -- deposit --user ${initKeypair.publicKey()} --amount ${xlmAmount}`);

  console.log(`Seeding USDC pool with ${usdcAmount / SCALAR_7} USDC...`);
  exe(`stellar contract invoke --id ${usdcPoolId} --source-account ${account} --network testnet -- deposit --user ${initKeypair.publicKey()} --amount ${usdcAmount}`);

  console.log(`Seeding EURC pool with ${eurcAmount / SCALAR_7} EURC...`);
  exe(`stellar contract invoke --id ${eurcPoolId} --source-account ${account} --network testnet -- deposit --user ${initKeypair.publicKey()} --amount ${eurcAmount}`);
};

export const fundFaucet = async (tokens: IssuedTokens, faucetAddress: string) => {
  const { issuer } = tokens;
  const FAUCET_FUND = 500_000;

  // Give faucet a trustline + tokens via SAC transfer (faucet is a contract, uses SAC)
  // Transfer from issuer to faucet using SAC contract invoke
  const amount = BigInt(FAUCET_FUND) * SCALAR_7;

  console.log(`Funding faucet with ${FAUCET_FUND} of each token...`);

  exe(`stellar contract invoke --id ${tokens.usdcAddress} --source-account ${issuer.secret()} --network testnet -- transfer --from ${issuer.publicKey()} --to ${faucetAddress} --amount ${amount}`);
  exe(`stellar contract invoke --id ${tokens.eurcAddress} --source-account ${issuer.secret()} --network testnet -- transfer --from ${issuer.publicKey()} --to ${faucetAddress} --amount ${amount}`);
  exe(`stellar contract invoke --id ${tokens.laiAddress} --source-account ${issuer.secret()} --network testnet -- transfer --from ${issuer.publicKey()} --to ${faucetAddress} --amount ${amount}`);
};
