import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import type { SupportedCurrency } from 'currencies';
import { type PropsWithChildren, createContext, useCallback, useContext, useEffect, useState } from 'react';
import { CURRENCY_INSURANCE_BINDINGS } from 'src/insurance-bindings';

export type PriceRecord = {
  [K in SupportedCurrency]: bigint;
};

export type InsurancePoolState = {
  totalShares: bigint;
  totalTokens: bigint;
};

export type InsurancePoolRecord = {
  [K in SupportedCurrency]: InsurancePoolState;
};

export type InsurancePoolContext = {
  prices: PriceRecord | null;
  pools: InsurancePoolRecord | null;
  refetchPools: () => void;
};

const Context = createContext<InsurancePoolContext>({
  prices: null,
  pools: null,
  refetchPools: () => { },
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

const fetchInsurancePools = async (): Promise<InsurancePoolRecord> => {
  const [XLM, USDC, EURC] = await Promise.all([fetchInsurancePoolState('XLM'), fetchInsurancePoolState('USDC'), fetchInsurancePoolState('EURC')]);
  return { XLM, USDC, EURC };
};

const fetchInsurancePoolState = async (ticker: SupportedCurrency): Promise<InsurancePoolState> => {
  const { contractClient } = CURRENCY_INSURANCE_BINDINGS[ticker];
  const { result } = await contractClient.get_pool_state();
  return {
    totalShares: result.total_shares,
    totalTokens: result.total_tokens,
  };
};

export const InsurancePoolProvider = ({ children }: PropsWithChildren) => {
  const [prices, setPrices] = useState<PriceRecord | null>(null);
  const [pools, setPools] = useState<InsurancePoolRecord | null>(null);

  const refetchPools = useCallback(() => {
    fetchAllPrices()
      .then((res) => setPrices(res))
      .catch((err) => console.error('Error fetching prices', err));
    fetchInsurancePools()
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

export const useInsurancePools = (): InsurancePoolContext => useContext(Context);
