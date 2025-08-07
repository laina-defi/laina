import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import type { SupportedCurrency } from 'currencies';
import { type PropsWithChildren, createContext, useCallback, useContext, useEffect, useState } from 'react';
import { CURRENCY_BINDINGS } from 'src/currency-bindings';

export type PriceRecord = {
  [K in SupportedCurrency]: bigint;
};

export type PoolState = {
  totalBalanceTokens: bigint;
  totalBalanceShares: bigint;
  availableBalanceTokens: bigint;
  annualInterestRate: bigint;
};

export type PoolRecord = {
  [K in SupportedCurrency]: PoolState;
};

export type PoolContext = {
  prices: PriceRecord | null;
  pools: PoolRecord | null;
  refetchPools: () => void;
};

const Context = createContext<PoolContext>({
  prices: null,
  pools: null,
  refetchPools: () => {},
});

const fetchAllPrices = async (): Promise<PriceRecord> => {
  const [XLM, USDC, EURC] = await Promise.all([fetchPriceData('XLM'), fetchPriceData('USDC'), fetchPriceData('EURC')]);
  return { XLM, USDC, EURC };
};

const fetchPriceData = async (ticker: string): Promise<bigint> => {
  const { result } = await loanManagerClient.get_price({ token: ticker });
  if (result.isOk()) {
    const value = result.unwrap();
    return value;
  }
  const error = result.unwrapErr();
  console.error('Error: ', error);
  return 0n;
};

const fetchPools = async (): Promise<PoolRecord> => {
  const [XLM, USDC, EURC] = await Promise.all([fetchPoolState('XLM'), fetchPoolState('USDC'), fetchPoolState('EURC')]);
  return { XLM, USDC, EURC };
};

const fetchPoolState = async (ticker: SupportedCurrency): Promise<PoolState> => {
  const { contractClient } = CURRENCY_BINDINGS[ticker];
  const { result } = await contractClient.get_pool_state();
  if (result.isOk()) {
    const value = result.unwrap();
    return {
      totalBalanceTokens: value.total_balance_tokens,
      totalBalanceShares: value.total_balance_shares,
      availableBalanceTokens: value.available_balance_tokens,
      annualInterestRate: value.annual_interest_rate,
    };
  }
  const error = result.unwrapErr();
  console.error('Error: ', error);
  return {
    totalBalanceTokens: 0n,
    totalBalanceShares: 0n,
    availableBalanceTokens: 0n,
    annualInterestRate: 0n,
  };
};

export const PoolProvider = ({ children }: PropsWithChildren) => {
  const [prices, setPrices] = useState<PriceRecord | null>(null);
  const [pools, setPools] = useState<PoolRecord | null>(null);

  const refetchPools = useCallback(() => {
    fetchAllPrices()
      .then((res) => setPrices(res))
      .catch((err) => console.error('Error fetching prices', err));
    fetchPools()
      .then((res) => setPools(res))
      .catch((err) => console.error('Error fetching pools', err));
  }, []);

  useEffect(() => {
    refetchPools();

    // Set up a timer for every ledger (~6 secs) to refetch state.
    const intervalId = setInterval(refetchPools, 6000);
    return () => clearInterval(intervalId);
  }, [refetchPools]);

  return <Context.Provider value={{ prices, pools, refetchPools }}>{children}</Context.Provider>;
};

export const usePools = (): PoolContext => useContext(Context);
