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

// Deposit APY: depositors earn on the utilised portion, after 10% protocol fee, split 50/50.
// APY = borrowRate × utilization × 0.9 × 0.5
export const calcDepositAPY = (
  annualInterestRate: bigint,
  totalBalanceTokens: bigint,
  availableBalanceTokens: bigint,
): number => {
  if (totalBalanceTokens === 0n) return 0;
  const utilization = Number(totalBalanceTokens - availableBalanceTokens) / Number(totalBalanceTokens);
  return (Number(annualInterestRate) / 100_000) * utilization * 0.9 * 0.5;
};

export const formatDepositAPY = (
  annualInterestRate: bigint,
  totalBalanceTokens: bigint,
  availableBalanceTokens: bigint,
): string => {
  return `${calcDepositAPY(annualInterestRate, totalBalanceTokens, availableBalanceTokens).toFixed(2)} %`;
};

// Insurance APY: insurers earn the same absolute interest as depositors, but their pool is
// typically much smaller, so APY = borrowRate × borrowed × 0.9 × 0.5 / insurancePoolTotal
export const calcInsuranceAPY = (
  annualInterestRate: bigint,
  totalLendingBalance: bigint,
  availableLendingBalance: bigint,
  totalInsuranceTokens: bigint,
): number => {
  if (totalInsuranceTokens === 0n) return 0;
  const borrowed = totalLendingBalance - availableLendingBalance;
  if (borrowed <= 0n) return 0;
  return (Number(annualInterestRate) / 100_000) * (Number(borrowed) / Number(totalInsuranceTokens)) * 0.9 * 0.5;
};

export const formatInsuranceAPY = (
  annualInterestRate: bigint,
  totalLendingBalance: bigint,
  availableLendingBalance: bigint,
  totalInsuranceTokens: bigint,
): string => {
  return `${calcInsuranceAPY(annualInterestRate, totalLendingBalance, availableLendingBalance, totalInsuranceTokens).toFixed(2)} %`;
};
