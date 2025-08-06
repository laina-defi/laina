import { readFileSync } from 'fs';
import { execSync } from 'child_process';

const [, , ticker, price, timestamp = '1'] = process.argv;

if (!ticker || !price) {
  console.error('Usage: npm run set-price <ticker> <price> [timestamp]');
  console.error('Example: npm run set-price XLM 12395743847612 1');
  process.exit(1);
}

try {
  const oracleContract = readFileSync('.stellar/contract-ids/reflector_oracle_mock.txt', 'utf8').trim();

  const command = `stellar contract invoke --id ${oracleContract} --network local --source-account ci_local -- update_price --asset '{"Other":"${ticker}"}' --price '{"price":"${price}","timestamp":${timestamp}}'`;

  console.log(`Setting ${ticker} price to ${price}...`);
  execSync(command, { stdio: 'inherit' });
  console.log(`✅ Set ${ticker} price to ${price} with timestamp ${timestamp}`);
} catch (error) {
  console.error('❌ Failed to set price:', error);
  process.exit(1);
}
