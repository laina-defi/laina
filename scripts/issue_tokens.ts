import 'dotenv/config';
import { Keypair, Horizon, TransactionBuilder, Networks, Operation, Asset, BASE_FEE } from '@stellar/stellar-sdk';
import { execSync } from 'child_process';
import { writeEnvVar } from './util';

const HORIZON_URL = 'https://horizon-testnet.stellar.org';
const FRIENDBOT_URL = 'https://friendbot.stellar.org';
const NETWORK = Networks.TESTNET;

const server = new Horizon.Server(HORIZON_URL);

const fund = async (address: string) => {
  const res = await fetch(`${FRIENDBOT_URL}?addr=${address}`);
  // 400 = account already exists, which is fine on re-runs
  if (!res.ok && res.status !== 400) throw new Error(`Friendbot failed for ${address}: ${res.statusText}`);
  console.log(`Funded ${address}`);
};

/** Load issuer keypair from env or generate + persist a new one */
const loadOrCreateIssuer = (): Keypair => {
  const secret = process.env.ISSUER_SECRET_KEY;
  if (secret) {
    console.log('Reusing existing issuer keypair from ISSUER_SECRET_KEY');
    return Keypair.fromSecret(secret);
  }
  const kp = Keypair.random();
  writeEnvVar('ISSUER_SECRET_KEY', kp.secret());
  console.log(`Created new issuer: ${kp.publicKey()}`);
  return kp;
};

const deploySAC = (ticker: string, issuerPublic: string, issuerSecret: string): string => {
  try {
    const address = execSync(
      `stellar contract asset deploy --asset ${ticker}:${issuerPublic} --network testnet --source-account ${issuerSecret}`,
      { encoding: 'utf-8', stdio: ['pipe', 'pipe', 'pipe'] },
    ).trim();
    console.log(`${ticker} SAC deployed: ${address}`);
    return address;
  } catch {
    // SAC already exists — its address is deterministic, just look it up
    const address = execSync(`stellar contract id asset --asset ${ticker}:${issuerPublic} --network testnet`, {
      encoding: 'utf-8',
    }).trim();
    console.log(`${ticker} SAC already exists: ${address}`);
    return address;
  }
};

export interface IssuedTokens {
  issuer: Keypair;
  usdcAddress: string;
  eurcAddress: string;
  laiAddress: string;
}

export const issueTokens = async (): Promise<IssuedTokens> => {
  const issuer = loadOrCreateIssuer();

  await fund(issuer.publicKey());
  // Small delay to let the account settle on the ledger
  await new Promise((r) => setTimeout(r, 3000));

  const account = await server.loadAccount(issuer.publicKey());
  const usdcAsset = new Asset('USDC', issuer.publicKey());
  const eurcAsset = new Asset('EURC', issuer.publicKey());
  const laiAsset = new Asset('LAI', issuer.publicKey());

  // Issuer can always pay themselves their own asset — idempotent on re-runs
  // LAI total supply is 100M (50M for distribution + 50M reserve/other uses)
  const mintTx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: NETWORK })
    .addOperation(Operation.payment({ destination: issuer.publicKey(), asset: usdcAsset, amount: '1000000000' }))
    .addOperation(Operation.payment({ destination: issuer.publicKey(), asset: eurcAsset, amount: '1000000000' }))
    .addOperation(Operation.payment({ destination: issuer.publicKey(), asset: laiAsset, amount: '100000000' }))
    .setTimeout(30)
    .build();

  mintTx.sign(issuer);
  await server.submitTransaction(mintTx);
  console.log('Minted 1B of USDC, EURC and 100M of LAI to issuer');

  // Small delay so the ledger reflects the minted balances before SAC deploy
  await new Promise((r) => setTimeout(r, 3000));

  const usdcAddress = deploySAC('USDC', issuer.publicKey(), issuer.secret());
  const eurcAddress = deploySAC('EURC', issuer.publicKey(), issuer.secret());
  const laiAddress = deploySAC('LAI', issuer.publicKey(), issuer.secret());

  writeEnvVar('PUBLIC_CONTRACT_ADDRESS_USDC', usdcAddress);
  writeEnvVar('PUBLIC_ISSUER_ADDRESS_USDC', issuer.publicKey());
  writeEnvVar('PUBLIC_CONTRACT_ADDRESS_EURC', eurcAddress);
  writeEnvVar('PUBLIC_ISSUER_ADDRESS_EURC', issuer.publicKey());
  writeEnvVar('PUBLIC_CONTRACT_ADDRESS_LAI', laiAddress);
  writeEnvVar('PUBLIC_ISSUER_ADDRESS_LAI', issuer.publicKey());

  return { issuer, usdcAddress, eurcAddress, laiAddress };
};

// Allow running standalone: node --import tsx scripts/issue_tokens.ts
if (process.argv[1]?.endsWith('issue_tokens.ts')) {
  issueTokens().then(({ issuer, usdcAddress, eurcAddress, laiAddress }) => {
    console.log(`\nIssuer: ${issuer.publicKey()}`);
    console.log(`USDC:   ${usdcAddress}`);
    console.log(`EURC:   ${eurcAddress}`);
    console.log(`LAI:    ${laiAddress}`);
  });
}
