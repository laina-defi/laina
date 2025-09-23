import { config } from 'dotenv';
import { readFileSync } from 'fs';
import { execSync } from 'child_process';
import { pathToFileURL } from 'url';

config();

const account = process.env.SOROBAN_ACCOUNT;

export const setPrice = (ticker: string, price: string, network: string, timestamp: string) => {
  try {
    const oracleContract = readFileSync('.stellar/contract-ids/reflector_oracle_mock.txt', 'utf8').trim();

    const command = `stellar contract invoke \
    --id ${oracleContract} \
    --network ${network} \
    --source-account ${account} \
    -- update_price \
    --asset '{"Other": "${ticker}"}' \
    --price '{"price": "${price}", "timestamp": ${timestamp}}'`;

    console.log(`Setting ${ticker} price to ${price}...`);
    execSync(command, { stdio: 'inherit' });
    console.log(`✅ Set ${ticker} price to ${price} with timestamp ${timestamp}`);
  } catch (error) {
    console.error('❌ Failed to set price:', error);
    process.exit(1);
  }
};

// ESM-compatible entrypoint check
if (import.meta.url === pathToFileURL(process.argv[1] as string).href) {
  const [, , ticker, price, network = 'local', timestamp = '1'] = process.argv;

  if (!ticker || !price) {
    console.error('Usage: npm run set-price <ticker> <price> [network] [timestamp]');
    console.error('Example: npm run set-price XLM 12395743847612 1');
    process.exit(1);
  }

  setPrice(ticker, price, network, timestamp);
}
