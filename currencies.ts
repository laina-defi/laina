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
  tokenContractAddress: 'CAHMBFPE4BNP26VUFRYBJ43GWENCAS2JAGQ7VPBV23CUFL4ZWZQGNBGO',
  loanPoolName: 'pool_usdc',
  issuer: 'GBE3CPBXTOGG75G7GETO5QZBYB4WCDTX6XWUEVZMXFP6Q66OR4MSLIPU',
} as const;

export const CURRENCY_EURC: Currency = {
  name: 'Euro Coin',
  ticker: 'EURC',
  issuerName: 'centre.io',
  tokenContractAddress: 'CDR3UKQ3L5K2JV2OINPVLB6NIOLSROAKMWPEML4CMXBN5NBAUGWFBNYZ',
  loanPoolName: 'pool_eurc',
  issuer: 'GBE3CPBXTOGG75G7GETO5QZBYB4WCDTX6XWUEVZMXFP6Q66OR4MSLIPU',
} as const;

export const CURRENCIES: Currency[] = [CURRENCY_XLM, CURRENCY_USDC, CURRENCY_EURC] as const;
