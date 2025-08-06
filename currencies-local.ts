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

// Local network addresses
export const CURRENCY_XLM: Currency = {
  name: 'Stellar Lumens',
  ticker: 'XLM',
  issuerName: 'native',
  tokenContractAddress: 'CDMLFMKMMD7MWZP3FKUBZPVHTUEDLSX4BYGYKH4GCESXYHS3IHQ4EIG4',
  loanPoolName: 'pool_xlm',
} as const;

export const CURRENCY_USDC: Currency = {
  name: 'USD Coin',
  ticker: 'USDC',
  issuerName: 'centre.io',
  tokenContractAddress: 'CDU4BRSZXYHAN3XINOJEKGUQ4WNLSLVQC5H6F6N75ORBU5YMNTPIJH7H',
  loanPoolName: 'pool_usdc',
  issuer: 'GAMX6CTD62UMM7EH24ULHZZWN3K3WI6BVHGQZE5HOCZSRBDNKT2J3I2U',
} as const;

export const CURRENCY_EURC: Currency = {
  name: 'Euro Coin',
  ticker: 'EURC',
  issuerName: 'centre.io',
  tokenContractAddress: 'CBA4EB6OXQOP3VOT36P7JX3ALWMRNGVEDIILIY42H5K7SKJTNXXM24FN',
  loanPoolName: 'pool_eurc',
  issuer: 'GAMX6CTD62UMM7EH24ULHZZWN3K3WI6BVHGQZE5HOCZSRBDNKT2J3I2U',
} as const;

export const CURRENCIES: Currency[] = [CURRENCY_XLM, CURRENCY_USDC, CURRENCY_EURC] as const;
