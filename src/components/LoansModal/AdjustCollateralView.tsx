import { Button } from '@components/Button';
import { CryptoAmountSelector } from '@components/CryptoAmountSelector';
import { ErrorDialogContent, LoadingDialogContent, SuccessDialogContent } from '@components/Dialog';
import { HealthFactor } from '@components/HealthFactor';
import { type Loan, useLoans } from '@contexts/loan-context';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import { getStroops } from '@lib/converters';
import { formatAmount, toCents } from '@lib/formatting';
import { useState } from 'react';
import { CURRENCY_BINDINGS } from 'src/currency-bindings';

const FIXED_POINT = 10_000_000n;

interface AdjustCollateralViewProps {
  loan: Loan;
  onBack: VoidFunction;
}

const AdjustCollateralView = ({ loan, onBack }: AdjustCollateralViewProps) => {
  const { collateralAmount, collateralTicker, borrowedAmount, borrowedTicker, unpaidInterest } = loan;
  const { wallet, walletBalances, signTransaction, refetchBalances } = useWallet();
  const { prices, pools } = usePools();
  const { refetchLoans } = useLoans();

  const [mode, setMode] = useState<'add' | 'remove'>('add');
  const [amount, setAmount] = useState(0n);
  const [isLoading, setIsLoading] = useState(false);
  const [success, setSuccess] = useState<string | null>(null);
  const [error, setError] = useState<Error | null>(null);

  const collateralPrice = prices?.[collateralTicker];
  const borrowedPrice = prices?.[borrowedTicker];
  const collateralPool = pools?.[collateralTicker];

  const balanceRecord = walletBalances?.[collateralTicker];
  const walletBalance = balanceRecord?.trustLine ? getStroops(balanceRecord.balanceLine.balance) : 0n;
  // XLM needs 3 XLM reserve
  const maxAdd =
    collateralTicker === 'XLM' ? (walletBalance > 30_000_000n ? walletBalance - 30_000_000n : 0n) : walletBalance;

  // Calculate preview health factor
  const loanTotal = borrowedAmount + unpaidInterest;
  const loanValueCents = borrowedPrice ? toCents(borrowedPrice, loanTotal) : undefined;

  let previewCollateral = collateralAmount;
  if (amount > 0n) {
    previewCollateral =
      mode === 'add' ? collateralAmount + amount : collateralAmount > amount ? collateralAmount - amount : 0n;
  }
  const previewCollateralCents = collateralPrice ? toCents(collateralPrice, previewCollateral) : undefined;

  const collateralFactor = collateralPool
    ? collateralPool.totalBalanceTokens > 0n
      ? 8_000_000n
      : 8_000_000n // default 80% — ideally fetched from contract
    : 8_000_000n;

  const previewHealthFactor =
    loanValueCents && loanValueCents > 0n && previewCollateralCents
      ? Number((previewCollateralCents * collateralFactor) / FIXED_POINT) / Number(loanValueCents)
      : 0;

  const removeWouldLiquidate = mode === 'remove' && previewHealthFactor < 1.0 && amount > 0n;

  const handleAdd = async () => {
    if (!wallet || amount === 0n) return;
    setIsLoading(true);
    try {
      const tx = await loanManagerClient.add_collateral({ loan_id: loan.loanId, amount });
      await tx.signAndSend({ signTransaction });
      setSuccess(`Added ${formatAmount(amount)} ${collateralTicker} as collateral.`);
      refetchLoans();
      refetchBalances();
    } catch (err) {
      setError(err as Error);
    }
    setIsLoading(false);
  };

  const handleRemove = async () => {
    if (!wallet || amount === 0n || removeWouldLiquidate) return;
    setIsLoading(true);
    try {
      const tx = await loanManagerClient.remove_collateral({ loan_id: loan.loanId, amount });
      await tx.signAndSend({ signTransaction });
      setSuccess(`Removed ${formatAmount(amount)} ${collateralTicker} from collateral.`);
      refetchLoans();
      refetchBalances();
    } catch (err) {
      setError(err as Error);
    }
    setIsLoading(false);
  };

  if (isLoading) {
    return (
      <LoadingDialogContent
        title="Adjusting collateral"
        subtitle={`${mode === 'add' ? 'Adding' : 'Removing'} ${formatAmount(amount)} ${collateralTicker}.`}
        buttonText="Back"
        onClick={onBack}
      />
    );
  }

  if (success) {
    return <SuccessDialogContent subtitle={success} buttonText="Back" onClick={onBack} />;
  }

  if (error) {
    return <ErrorDialogContent error={error} onClick={onBack} />;
  }

  const { name } = CURRENCY_BINDINGS[collateralTicker];

  return (
    <div className="md:w-[700px]">
      <h3 className="text-xl font-bold tracking-tight">Adjust {name} Collateral</h3>
      <p className="my-4">
        Your collateral earns interest while locked. Add more to improve your health factor, or remove some to free up
        funds.
      </p>
      <p className="mb-4">
        Current collateral: {formatAmount(collateralAmount)} {collateralTicker}
        {collateralPrice && (
          <span className="text-grey-dark ml-2">
            (${(Number(toCents(collateralPrice, collateralAmount)) / 100).toFixed(2)})
          </span>
        )}
      </p>

      <div className="tabs tabs-boxed mb-6">
        <button
          type="button"
          className={`tab ${mode === 'add' ? 'tab-active' : ''}`}
          onClick={() => {
            setMode('add');
            setAmount(0n);
          }}
        >
          Add Collateral
        </button>
        <button
          type="button"
          className={`tab ${mode === 'remove' ? 'tab-active' : ''}`}
          onClick={() => {
            setMode('remove');
            setAmount(0n);
          }}
        >
          Remove Collateral
        </button>
      </div>

      <p className="font-bold mb-2">{mode === 'add' ? 'Amount to add' : 'Amount to remove'}</p>
      <CryptoAmountSelector
        max={mode === 'add' ? maxAdd : collateralAmount}
        value={amount}
        valueCents={collateralPrice ? toCents(collateralPrice, amount) : undefined}
        ticker={collateralTicker}
        onChange={setAmount}
        onSelectMaximum={() => setAmount(mode === 'add' ? maxAdd : collateralAmount)}
      />

      {amount > 0n && (
        <div className="mt-6">
          <p className="font-bold mb-2">Health factor preview</p>
          <HealthFactor value={previewHealthFactor} />
          {removeWouldLiquidate && (
            <p className="text-red mt-2 text-sm">Removing this amount would make your loan immediately liquidatable.</p>
          )}
        </div>
      )}

      <div className="flex flex-row justify-end mt-8 gap-4">
        <Button onClick={onBack} variant="ghost">
          Back
        </Button>
        {mode === 'add' ? (
          <Button disabled={amount === 0n} onClick={handleAdd}>
            Add Collateral
          </Button>
        ) : (
          <Button disabled={amount === 0n || removeWouldLiquidate} onClick={handleRemove}>
            Remove Collateral
          </Button>
        )}
      </div>
    </div>
  );
};

export default AdjustCollateralView;
