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

// The addresses here are for testnet.
// TODO: use environment variables for the addresses.

export const CURRENCY_XLM: Currency = {
  name: 'Stellar Lumens',
  ticker: 'XLM',
  issuerName: 'native',
  tokenContractAddress: 'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC',
  loanPoolName: 'pool_xlm',
} as const;

export const CURRENCY_USDC: Currency = {
  name: 'USD Coin',
  ticker: 'USDC',
  issuerName: 'centre.io',
  tokenContractAddress: 'CCFOGKTM7CC33YW2H5XSGJN5DLNXG2GUKA3XMRYRWZSY2G5QGW2V3PSO',
  loanPoolName: 'pool_usdc',
  issuer: 'GCAZVAXHZDFF25GIKG3XMUJKMB2IRFUD3N35GUQZMZA5GT66NYHRXBHO',
} as const;

export const CURRENCY_EURC: Currency = {
  name: 'Euro Coin',
  ticker: 'EURC',
  issuerName: 'centre.io',
  tokenContractAddress: 'CCH2DZNCAG72ARUIMYBAEWM7TXVCB4O32GG3RHULN5GRAP4HE5M4H4XP',
  loanPoolName: 'pool_eurc',
  issuer: 'GCAZVAXHZDFF25GIKG3XMUJKMB2IRFUD3N35GUQZMZA5GT66NYHRXBHO',
} as const;

export const CURRENCIES: Currency[] = [CURRENCY_XLM, CURRENCY_USDC, CURRENCY_EURC] as const;
