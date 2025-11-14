// 7 decimal numbers is the smallest unit of XLM, stroop.
export const SCALAR_7 = 10_000_000n;
export const CENTS_SCALAR = SCALAR_7 * SCALAR_7 * 100_000n;

const TEN_K = 10_000n * SCALAR_7;
const ONE_M = 1_000_000n * SCALAR_7;

// 10 thousand dollars = 1 million cents
const TEN_K_CENTS = 10_000n * 100n;
// 1 million dollars = 100 million cents
const ONE_M_CENTS = 1_000_000n * 100n;

export const formatAmount = (amount: bigint): string => {
  if (amount === 0n) return '0';

  if (amount > ONE_M) {
    return `${(Number(amount) / (1_000_000 * 10_000_000)).toFixed(2)}M`;
  }
  if (amount > TEN_K) {
    return `${(Number(amount) / (1_000 * 10_000_000)).toFixed(1)}K`;
  }
  return `${(Number(amount) / 10_000_000).toFixed(1)}`;
};

export const toDollarsFormatted = (price: bigint, amount: bigint): string => {
  if (amount === 0n) return '$0';
  return formatCentAmount(toCents(price, amount));
};

export const toCents = (price: bigint, amount: bigint): bigint => {
  return (price * amount) / CENTS_SCALAR;
};

export const fromCents = (price: bigint, cents: bigint): bigint => {
  return (cents * CENTS_SCALAR) / price;
};

export const formatCentAmount = (cents: bigint): string => {
  if (cents === 0n) return '$0';

  if (cents > ONE_M_CENTS) {
    return `$${(Number(cents) / 100_000_000).toFixed(2)} M`;
  }
  if (cents > TEN_K_CENTS) {
    return `$${(Number(cents) / 100_000).toFixed(2)} K`;
  }
  return `$${(Number(cents) / 100).toFixed(2)} `;
};

export const formatAPR = (apr: bigint): string => {
  return `${(Number(apr) / 100_000).toFixed(2)} %`;
};

export const formatAPY = (apr: bigint): string => {
  return `${(Number(apr) / 100_000).toFixed(2)} %`;
};
