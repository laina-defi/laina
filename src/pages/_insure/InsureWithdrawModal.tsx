import { useState } from 'react';

import { Button } from '@components/Button';
import { CryptoAmountSelector } from '@components/CryptoAmountSelector';
import { Dialog, ErrorDialogContent, LoadingDialogContent, SuccessDialogContent } from '@components/Dialog';
import { useInsurancePools } from '@contexts/insurance-pool-context';
import { useWallet } from '@contexts/wallet-context';
import { stroopsToDecimalString } from '@lib/converters';
import { toCents } from '@lib/formatting';
import type { CurrencyBinding } from 'src/insurance-bindings';

export interface InsureWithdrawModalProps {
  modalId: string;
  onClose: () => void;
  currency: CurrencyBinding | null;
}

export const InsureWithdrawModal = ({ modalId, onClose, currency }: InsureWithdrawModalProps) => {
  const { sendTransaction, isWithdrawing, isWithdrawSuccess, withdrawError, resetState } =
    useWithdrawTransaction(currency);
  const { wallet, insurancePositions, refetchBalances } = useWallet();
  const { prices } = useInsurancePools();
  const [amount, setAmount] = useState(0n);

  if (!currency) {
    return <Dialog modalId={modalId} onClose={onClose} />;
  }

  const { name, ticker } = currency;

  const insuranceBalance = (wallet && insurancePositions?.[ticker]) ?? 0n;
  const price = prices?.[ticker];
  const amountCents = price ? toCents(price, amount) : undefined;

  const closeModal = () => {
    refetchBalances();
    setAmount(0n);
    resetState();
    onClose();
  };

  const isWithdrawDisabled = amount === 0n || amount > insuranceBalance;

  if (isWithdrawing) {
    return (
      <Dialog
        modalId={modalId}
        onClose={() => {
          /* Disallow closing */
        }}
      >
        <LoadingDialogContent
          title="Withdrawing"
          subtitle={`Withdrawing ${stroopsToDecimalString(amount)} l${ticker}.`}
          onClick={closeModal}
        />
      </Dialog>
    );
  }

  if (isWithdrawSuccess) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <SuccessDialogContent
          subtitle={`Successfully withdrew ${stroopsToDecimalString(amount)} l${ticker}.`}
          onClick={closeModal}
        />
      </Dialog>
    );
  }

  if (withdrawError) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <ErrorDialogContent error={withdrawError} onClick={closeModal} />
      </Dialog>
    );
  }

  return (
    <Dialog className="min-w-[760px]" modalId={modalId} onClose={closeModal}>
      <h3 className="font-bold text-xl mb-2">Withdraw l{name}</h3>
      <p className="text-sm opacity-70 mb-8">
        Withdraw your l{ticker} tokens from the {name} insurance pool.
      </p>
      <p className="text-lg mb-2">Amount to withdraw</p>
      <CryptoAmountSelector
        max={insuranceBalance}
        value={amount}
        valueCents={amountCents}
        ticker={ticker}
        onChange={setAmount}
        onSelectMaximum={() => setAmount(insuranceBalance)}
      />

      <div className="flex flex-row justify-end mt-8">
        <Button onClick={closeModal} variant="ghost" className="mr-4">
          Cancel
        </Button>
        <Button disabled={isWithdrawDisabled} onClick={() => sendTransaction()}>
          Withdraw
        </Button>
      </div>
    </Dialog>
  );
};

const useWithdrawTransaction = (currency: CurrencyBinding | null) => {
  const { wallet, signTransaction } = useWallet();
  const [isWithdrawing, setIsWithdrawing] = useState(false);
  const [isWithdrawSuccess, setIsWithdrawSuccess] = useState(false);
  const [withdrawError, setWithdrawError] = useState<Error | null>(null);

  const resetState = () => {
    setIsWithdrawing(false);
    setIsWithdrawSuccess(false);
    setWithdrawError(null);
  };

  const sendTransaction = async () => {
    if (!wallet) {
      alert('Please connect your wallet first!');
      return;
    }
    if (!currency) {
      alert('No currency selected');
      return;
    }

    setIsWithdrawing(true);

    const tx = await currency.contractClient.execute_withdraw({
      user: wallet.address,
    });

    try {
      await tx.signAndSend({ signTransaction });
      setIsWithdrawSuccess(true);
      setWithdrawError(null);
    } catch (err) {
      setWithdrawError(err as Error);
      setIsWithdrawSuccess(false);
    }
    setIsWithdrawing(false);
  };

  return { isWithdrawing, isWithdrawSuccess, withdrawError, sendTransaction, resetState };
};
