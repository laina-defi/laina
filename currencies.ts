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
  tokenContractAddress: 'CBCM4ACDLAIG5VNG2UQCXYO5DCQO4VCPUUNRS3FOVBIPQIG73NQVWLPP',
  loanPoolName: 'pool_usdc',
  issuer: 'GCBLC2EMFHHODLPKTM4DGXKGU66KDGY3B4UL4R2UFNC7UVP6N3DB5DV5',
} as const;

export const CURRENCY_EURC: Currency = {
  name: 'Euro Coin',
  ticker: 'EURC',
  issuerName: 'centre.io',
  tokenContractAddress: 'CDUVQWSSXV2QR35ONLT3FWLGEO6TKENZUMUKXE5BVT3YKCRXRZJDPX4J',
  loanPoolName: 'pool_eurc',
  issuer: 'GCBLC2EMFHHODLPKTM4DGXKGU66KDGY3B4UL4R2UFNC7UVP6N3DB5DV5',
} as const;

export const CURRENCIES: Currency[] = [CURRENCY_XLM, CURRENCY_USDC, CURRENCY_EURC] as const;
