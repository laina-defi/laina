import { useWallet } from '@contexts/wallet-context';
import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import type { SupportedCurrency } from 'currencies';
import { type PropsWithChildren, createContext, useCallback, useContext, useEffect, useState } from 'react';
import { CURRENCY_BINDINGS_BY_ADDRESS, type PoolAddress } from 'src/currency-bindings';

export type Loan = {
  borrower: string;
  borrowedAmount: bigint;
  borrowedTicker: SupportedCurrency;
  collateralAmount: bigint;
  collateralTicker: SupportedCurrency;
  healthFactor: bigint;
  unpaidInterest: bigint;
};

export type LoansContext = {
  loans: Loan[] | null;
  refetchLoans: VoidFunction;
};

const Context = createContext<LoansContext>({
  loans: [],
  refetchLoans: () => {},
});

export const LoansProvider = ({ children }: PropsWithChildren) => {
  const [loans, setLoans] = useState<Loan[] | null>(null);
  const { wallet } = useWallet();

  // TODO: add support for having more than 1 loan
  const refetchLoans = useCallback(async () => {
    setLoans(null);
    if (!wallet) {
      return;
    }
    try {
      const { result } = await loanManagerClient.get_loan({ user: wallet.address });
      if (result) {
        setLoans([
          {
            borrower: result.value.borrower,
            borrowedAmount: result.value.borrowed_amount,
            borrowedTicker: CURRENCY_BINDINGS_BY_ADDRESS[result.value.borrowed_from as PoolAddress].ticker,
            collateralAmount: result.value.collateral_amount,
            collateralTicker: CURRENCY_BINDINGS_BY_ADDRESS[result.value.collateral_from as PoolAddress].ticker,
            healthFactor: result.value.health_factor,
            unpaidInterest: result.value.unpaid_interest,
          },
        ]);
      } else {
        setLoans([]);
      }
    } catch (err) {
      console.error('Error fetching user loan:', err);
      setLoans([]);
    }
  }, [wallet]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: We want to synchronise loans & wallet
  useEffect(() => {
    refetchLoans();
  }, [refetchLoans, wallet]);

  return <Context.Provider value={{ loans, refetchLoans }}>{children}</Context.Provider>;
};

export const useLoans = (): LoansContext => useContext(Context);
