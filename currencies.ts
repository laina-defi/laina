import dotenv from 'dotenv';

export type SupportedCurrency = 'XLM' | 'USDC' | 'EURC';

export const isSupportedCurrency = (obj: unknown): obj is SupportedCurrency =>
  typeof obj === 'string' && ['XLM', 'USDC', 'EURC'].includes(obj);

export type Currency = {
  name: string;
  ticker: SupportedCurrency;
  issuerName: string;
  tokenContractAddress: string;
  loanPoolName: string;
  issuer?: string;
};

  const envConfig = {
    CONTRACT_ADDRESS_XLM: getEnvVar('PUBLIC_CONTRACT_ADDRESS_XLM'),
    CONTRACT_ADDRESS_USDC: getEnvVar('PUBLIC_CONTRACT_ADDRESS_USDC'),
    CONTRACT_ADDRESS_EURC: getEnvVar('PUBLIC_CONTRACT_ADDRESS_EURC'),
    ISSUER_ADDRESS_USDC: getEnvVar('PUBLIC_ISSUER_ADDRESS_USDC'),
    ISSUER_ADDRESS_EURC: getEnvVar('PUBLIC_ISSUER_ADDRESS_EURC'),
  } as const;

// Utility function to get environment variables from either meta.env (Astro) or process.env (Node.js)
function getEnvVar(key: string): string {
  // Check if we're in an Astro environment (meta.env is available)
  if (typeof import.meta !== 'undefined' && import.meta.env) {
    const value = import.meta.env[key];
    if (value !== undefined) {
      return value;
    }
  }

  // Fall back to process.env (Node.js environment)
  if (typeof process !== 'undefined' && process.env) {
    dotenv.config(); // Uses DOTENV_CONFIG_PATH or defaults to .env

    const value = process.env[key];
    if (value !== undefined) {
      return value;
    }
  }

  // If neither is available, throw an error
  throw new Error(`Environment variable ${key} is not set`);
}

export const CURRENCY_XLM: Currency = {
  name: 'Stellar Lumens',
  ticker: 'XLM',
  issuerName: 'native',
  tokenContractAddress: envConfig.CONTRACT_ADDRESS_XLM,
  loanPoolName: 'pool_xlm',
} as const;

export const CURRENCY_USDC: Currency = {
  name: 'USD Coin',
  ticker: 'USDC',
  issuerName: 'centre.io',
  tokenContractAddress: envConfig.CONTRACT_ADDRESS_USDC,
  loanPoolName: 'pool_usdc',
  issuer: envConfig.ISSUER_ADDRESS_USDC,
} as const;

export const CURRENCY_EURC: Currency = {
  name: 'Euro Coin',
  ticker: 'EURC',
  issuerName: 'centre.io',
  tokenContractAddress: envConfig.CONTRACT_ADDRESS_EURC,
  loanPoolName: 'pool_eurc',
  issuer: envConfig.ISSUER_ADDRESS_EURC,
} as const;

export const CURRENCIES: Currency[] = [CURRENCY_XLM, CURRENCY_USDC, CURRENCY_EURC] as const;
