import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import type { SupportedCurrency } from 'currencies';
import { type PropsWithChildren, createContext, useCallback, useContext, useEffect, useState } from 'react';
import { CURRENCY_BINDINGS_BY_ADDRESS, type PoolAddress } from 'src/currency-bindings';

export type Loan = {
  loanId: LoanId;
  borrowedAmount: bigint;
  borrowedTicker: SupportedCurrency;
  /** Current token value of collateral (shares converted at current pool rate, earns interest) */
  collateralAmount: bigint;
  collateralShares: bigint;
  collateralTicker: SupportedCurrency;
  healthFactor: bigint;
  unpaidInterest: bigint;
};

export type LoanId = {
  borrower_address: string;
  nonce: bigint;
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
  const { pools } = usePools();

  const refetchLoans = useCallback(async () => {
    if (!wallet) {
      setLoans(null);
      return;
    }
    try {
      const { result } = await loanManagerClient.get_loans({ user: wallet.address });
      const mappedLoans = result.map((loan) => {
        const collateralTicker = CURRENCY_BINDINGS_BY_ADDRESS[loan.collateral_from as PoolAddress].ticker;
        const collateralShares = loan.collateral_shares;

        // Convert collateral shares to token value using current pool state
        let collateralAmount = collateralShares;
        if (pools) {
          const collateralPool = pools[collateralTicker];
          if (collateralPool && collateralPool.totalBalanceShares > 0n) {
            collateralAmount =
              (collateralShares * collateralPool.totalBalanceTokens) / collateralPool.totalBalanceShares;
          }
        }

        return {
          loanId: loan.loan_id,
          borrowedAmount: loan.borrowed_amount,
          borrowedTicker: CURRENCY_BINDINGS_BY_ADDRESS[loan.borrowed_from as PoolAddress].ticker,
          collateralAmount,
          collateralShares,
          collateralTicker,
          healthFactor: loan.health_factor,
          unpaidInterest: loan.unpaid_interest,
        };
      });
      setLoans(mappedLoans);
    } catch (err) {
      console.error('Error fetching user loan:', err);
      setLoans([]);
    }
  }, [wallet, pools]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: We want to synchronise loans & wallet
  useEffect(() => {
    refetchLoans();
  }, [refetchLoans, wallet]);

  return <Context.Provider value={{ loans, refetchLoans }}>{children}</Context.Provider>;
};

export const useLoans = (): LoansContext => useContext(Context);
