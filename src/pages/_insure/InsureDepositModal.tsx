import { useState } from 'react';

import { Button } from '@components/Button';
import { CryptoAmountSelector } from '@components/CryptoAmountSelector';
import { Dialog, ErrorDialogContent, LoadingDialogContent, SuccessDialogContent } from '@components/Dialog';
import { useInsurancePools } from '@contexts/insurance-pool-context';
import { useWallet } from '@contexts/wallet-context';
import { stroopsToDecimalString } from '@lib/converters';
import { toCents } from '@lib/formatting';
import type { CurrencyBinding } from 'src/insurance-bindings';

export interface InsureDepositModalProps {
  modalId: string;
  onClose: () => void;
  currency: CurrencyBinding | null;
}

export const InsureDepositModal = ({ modalId, onClose, currency }: InsureDepositModalProps) => {
  const { sendTransaction, isDepositing, isDepositSuccess, depositError, resetState } = useDepositTransaction(currency);
  const { positions, refetchBalances } = useWallet();
  const { prices } = useInsurancePools();
  const [amount, setAmount] = useState(0n);

  if (!currency) {
    return <Dialog modalId={modalId} onClose={onClose} />;
  }

  const { name, ticker } = currency;

  const lTokenBalance = positions?.[ticker]?.receivable_shares ?? 0n;
  const price = prices?.[ticker];
  const amountCents = price ? toCents(price, amount) : undefined;

  const closeModal = () => {
    refetchBalances();
    setAmount(0n);
    resetState();
    onClose();
  };

  const handleDepositClick = () => {
    sendTransaction(amount);
  };

  const handleAmountChange = (stroops: bigint) => {
    setAmount(stroops);
  };

  const handleSelectMax = () => {
    setAmount(lTokenBalance);
  };

  const isDepositDisabled = amount === 0n || amount > lTokenBalance;

  if (isDepositing) {
    return (
      <Dialog
        modalId={modalId}
        onClose={() => {
          /* Disallow closing */
        }}
      >
        <LoadingDialogContent
          title="Depositing"
          subtitle={`Depositing ${stroopsToDecimalString(amount)} l${ticker}.`}
          onClick={closeModal}
        />
      </Dialog>
    );
  }

  if (isDepositSuccess) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <SuccessDialogContent
          subtitle={`Successfully deposited ${stroopsToDecimalString(amount)} l${ticker}.`}
          onClick={closeModal}
        />
      </Dialog>
    );
  }

  if (depositError) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <ErrorDialogContent error={depositError} onClick={closeModal} />
      </Dialog>
    );
  }

  return (
    <Dialog className="min-w-[760px]" modalId={modalId} onClose={closeModal}>
      <h3 className="font-bold text-xl mb-2">Deposit l{name}</h3>
      <p className="text-sm opacity-70 mb-8">Deposit your l{ticker} tokens to provide insurance coverage for the {name} pool.</p>
      <p className="text-lg mb-2">Amount to deposit</p>
      <CryptoAmountSelector
        max={lTokenBalance}
        value={amount}
        valueCents={amountCents}
        ticker={ticker}
        onChange={handleAmountChange}
        onSelectMaximum={handleSelectMax}
      />

      <div className="flex flex-row justify-end mt-8">
        <Button onClick={closeModal} variant="ghost" className="mr-4">
          Cancel
        </Button>
        <Button disabled={isDepositDisabled} onClick={handleDepositClick}>
          Deposit
        </Button>
      </div>
    </Dialog>
  );
};

const useDepositTransaction = (currency: CurrencyBinding | null) => {
  const { wallet, signTransaction } = useWallet();
  const [isDepositing, setIsDepositing] = useState(false);
  const [isDepositSuccess, setIsDepositSuccess] = useState(false);
  const [depositError, setDepositError] = useState<Error | null>(null);

  const resetState = () => {
    setIsDepositing(false);
    setIsDepositSuccess(false);
    setDepositError(null);
  };

  const sendTransaction = async (stroops: bigint) => {
    if (!wallet) {
      alert('Please connect your wallet first!');
      return;
    }
    if (!currency) {
      alert('No currency selected');
      return;
    }

    setIsDepositing(true);

    const tx = await currency.contractClient.deposit({
      user: wallet.address,
      amount: stroops,
    });

    try {
      await tx.signAndSend({ signTransaction });

      setIsDepositSuccess(true);
      setDepositError(null);
    } catch (err) {
      setDepositError(err as Error);
      setIsDepositSuccess(false);
    }
    setIsDepositing(false);
  };

  return { isDepositing, isDepositSuccess, depositError, sendTransaction, resetState };
};
