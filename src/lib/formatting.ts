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

export const formatCollateralFactor = (factor: bigint): string =>
  `${((Number(factor) / 10_000_000) * 100).toFixed(0)} %`;

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

// TODO: LAI price is hard-coded at $0.01 — replace with a real price feed when available
const LAI_PRICE_USD = 0.01;
// 50M LAI distributed over 5 years at $0.01/LAI = $100K/yr across all pools
const LAI_YEARLY_USD = (50_000_000 * LAI_PRICE_USD) / 5;
const LAI_NUM_POOLS = 3; // XLM, USDC, EURC

// LAI APR earned by borrowers in a single pool, as a percentage.
// Borrowers receive 30% of annual LAI emissions, split equally across pools.
// APR = (LAI USD value per year for this pool side) / (total borrowed USD value)
export const calcBorrowerLaiAPR = (
  totalBalanceTokens: bigint,
  availableBalanceTokens: bigint,
  tokenPrice: bigint,
): number => {
  const borrowed = totalBalanceTokens - availableBalanceTokens;
  if (borrowed <= 0n || tokenPrice === 0n) return 0;
  const laiPoolYearlyUSD = (LAI_YEARLY_USD * 0.3) / LAI_NUM_POOLS;
  const borrowedUSD = Number(toCents(tokenPrice, borrowed)) / 100;
  if (borrowedUSD === 0) return 0;
  return (laiPoolYearlyUSD / borrowedUSD) * 100;
};

// LAI APR earned by insurers in a single pool, as a percentage.
// Insurers receive 70% of annual LAI emissions, split equally across pools.
// APR = (LAI USD value per year for this pool side) / (total insurer deposit USD value)
export const calcInsurerLaiAPR = (totalInsuranceTokens: bigint, tokenPrice: bigint): number => {
  if (totalInsuranceTokens === 0n || tokenPrice === 0n) return 0;
  const laiPoolYearlyUSD = (LAI_YEARLY_USD * 0.7) / LAI_NUM_POOLS;
  const insurerUSD = Number(toCents(tokenPrice, totalInsuranceTokens)) / 100;
  if (insurerUSD === 0) return 0;
  return (laiPoolYearlyUSD / insurerUSD) * 100;
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
