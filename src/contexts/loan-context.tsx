import { useWallet } from '@contexts/wallet-context';
import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import type { SupportedCurrency } from 'currencies';
import { type PropsWithChildren, createContext, useCallback, useContext, useEffect, useState } from 'react';
import { CURRENCY_BINDINGS_BY_ADDRESS, type PoolAddress } from 'src/currency-bindings';

export type Loan = {
  loanId: LoanId;
  borrowedAmount: bigint;
  borrowedTicker: SupportedCurrency;
  collateralAmount: bigint;
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

  // TODO: add support for having more than 1 loan
  const refetchLoans = useCallback(async () => {
    setLoans(null);
    if (!wallet) {
      return;
    }
    try {
      const { result } = await loanManagerClient.get_loans({ user: wallet.address });
      const loans = result.map((loan) => ({
        loanId: loan.loan_id,
        borrowedAmount: loan.borrowed_amount,
        borrowedTicker: CURRENCY_BINDINGS_BY_ADDRESS[loan.borrowed_from as PoolAddress].ticker,
        collateralAmount: loan.collateral_amount,
        collateralTicker: CURRENCY_BINDINGS_BY_ADDRESS[loan.collateral_from as PoolAddress].ticker,
        healthFactor: loan.health_factor,
        unpaidInterest: loan.unpaid_interest,
      }));
      setLoans(loans);
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
