import { Button } from '@components/Button';
import { CryptoAmountSelector } from '@components/CryptoAmountSelector';
import { ErrorDialogContent, LoadingDialogContent, SuccessDialogContent } from '@components/Dialog';
import { type Loan, useLoans } from '@contexts/loan-context';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import { formatAPR, formatAmount, toCents } from '@lib/formatting';
import { useState } from 'react';
import { CURRENCY_BINDINGS } from 'src/currency-bindings';

interface RepayViewProps {
  loan: Loan;
  onBack: VoidFunction;
}

const RepayView = ({ loan, onBack }: RepayViewProps) => {
  const { borrowedAmount, borrowedTicker, collateralAmount, collateralTicker, unpaidInterest } = loan;
  const { name } = CURRENCY_BINDINGS[borrowedTicker];
  const { wallet, signTransaction, refetchBalances } = useWallet();
  const { prices, pools } = usePools();
  const { refetchLoans } = useLoans();
  const [amount, setAmount] = useState(0n);
  const [isRepaying, setIsRepaying] = useState(false);
  const [isRepayingAll, setIsRepayingAll] = useState(false);
  const [success, setSuccess] = useState<'PARTIAL_REPAY_SUCCESS' | 'FULL_REPAY_SUCCESS' | null>(null);
  const [error, setError] = useState<Error | null>(null);

  const loanBalance = borrowedAmount + unpaidInterest;
  const apr = pools?.[borrowedTicker]?.annualInterestRate;
  const price = prices?.[borrowedTicker];
  const valueCents = price ? toCents(price, amount) : undefined;

  const handleAmountChange = (stroops: bigint) => {
    setAmount(stroops);
  };

  const handleSelectMax = () => {
    setAmount(loanBalance);
  };

  const handleRepayClick = async () => {
    if (!wallet) return;

    setIsRepaying(true);

    const tx = await loanManagerClient.repay({ loan_id: loan.loanId, amount });
    try {
      await tx.signAndSend({ signTransaction });
      setSuccess('PARTIAL_REPAY_SUCCESS');
    } catch (err) {
      console.error('Error repaying', err);
      setError(err as Error);
    }
    refetchLoans();
    refetchBalances();
    setIsRepaying(false);
  };

  const handleRepayAllClick = async () => {
    if (!wallet) return;

    setIsRepayingAll(true);

    const tx = await loanManagerClient.repay_and_close_manager({
      loan_id: loan.loanId,
      // +5% to liabilities. TEMPORARY hard-coded solution for max allowance.
      max_allowed_amount: (loanBalance * 5n) / 100n + loanBalance,
    });
    try {
      await tx.signAndSend({ signTransaction });
      setSuccess('FULL_REPAY_SUCCESS');
    } catch (err) {
      console.error('Error repaying', err);
      setError(err as Error);
    }
    refetchLoans();
    refetchBalances();
    setIsRepayingAll(false);
  };

  if (isRepaying) {
    return (
      <LoadingDialogContent
        title="Repaying"
        subtitle={`Repaying ${formatAmount(amount)} ${borrowedTicker}.`}
        buttonText="Back"
        onClick={onBack}
      />
    );
  }

  if (isRepayingAll) {
    return (
      <LoadingDialogContent
        title="Repaying"
        subtitle={`Repaying ${formatAmount(loanBalance)} ${borrowedTicker}, all of your outstanding loan.`}
        buttonText="Back"
        onClick={onBack}
      />
    );
  }

  if (success) {
    const subtitle =
      success === 'FULL_REPAY_SUCCESS'
        ? `Successfully repaid all of your loan. The collateral ${formatAmount(collateralAmount)} ${collateralTicker} was returned back to your wallet.`
        : `Successfully repaid ${formatAmount(amount)} ${borrowedTicker}.`;

    return <SuccessDialogContent subtitle={subtitle} buttonText="Back" onClick={onBack} />;
  }

  if (error) {
    return <ErrorDialogContent error={error} onClick={onBack} />;
  }

  return (
    <div className="md:w-[700px]">
      <h3 className="text-xl font-bold tracking-tight">Repay {name}</h3>
      <p className="my-4">
        Repay some or all of your loan. Repaying the loan in full will return the collateral back to you.
      </p>
      {apr && <p className="mb-4">The annual interest rate is currently {formatAPR(apr)}.</p>}
      <p>
        Borrowed amount: {formatAmount(loanBalance)} {borrowedTicker}
      </p>
      <p>
        Collateral: {formatAmount(collateralAmount)} {collateralTicker}
      </p>
      <p className="font-bold mb-2 mt-6">Select the amount to repay</p>
      <CryptoAmountSelector
        max={loanBalance}
        value={amount}
        valueCents={valueCents}
        ticker={borrowedTicker}
        onChange={handleAmountChange}
        onSelectMaximum={handleSelectMax}
      />
      <div className="flex flex-row justify-end mt-8 gap-4">
        <Button onClick={onBack} variant="ghost">
          Back
        </Button>
        <Button disabled={amount === 0n} onClick={handleRepayClick}>
          Repay
        </Button>
        <Button onClick={handleRepayAllClick}>Repay All</Button>
      </div>
    </div>
  );
};

export default RepayView;
