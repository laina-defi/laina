import { useState } from 'react';

import { Button } from '@components/Button';
import { CryptoAmountSelector } from '@components/CryptoAmountSelector';
import { Dialog, ErrorDialogContent, LoadingDialogContent, SuccessDialogContent } from '@components/Dialog';
import { QueueProgressBar } from '@components/QueueProgressBar';
import { useInsurancePools } from '@contexts/insurance-pool-context';
import { useWallet } from '@contexts/wallet-context';
import { stroopsToDecimalString } from '@lib/converters';
import { formatAmount, toCents } from '@lib/formatting';
import { isNil } from 'ramda';
import type { CurrencyBinding } from 'src/insurance-bindings';

const FOURTEEN_DAYS_IN_SECONDS = 14 * 24 * 60 * 60;

export interface InsureManageModalProps {
  modalId: string;
  onClose: () => void;
  currency: CurrencyBinding | null;
}

type Tab = 'deposit' | 'withdraw';

export const InsureManageModal = ({ modalId, onClose, currency }: InsureManageModalProps) => {
  const [tab, setTab] = useState<Tab>('deposit');

  const deposit = useDepositTransaction(currency);
  const queueWithdraw = useQueueWithdrawTransaction(currency);
  const cancelQueue = useCancelQueueTransaction(currency);
  const executeWithdraw = useExecuteWithdrawTransaction(currency);

  const { wallet, positions, insurancePositions, insuranceQueues, refetchBalances } = useWallet();
  const { prices, pools } = useInsurancePools();

  const [depositAmount, setDepositAmount] = useState(0n);
  const [withdrawAmount, setWithdrawAmount] = useState(0n);

  if (!currency) {
    return <Dialog modalId={modalId} onClose={onClose} />;
  }

  const { name, ticker } = currency;
  const lTokenBalance = positions?.[ticker]?.receivable_shares ?? 0n;
  const insuranceBalance = (wallet && insurancePositions?.[ticker]) ?? 0n;
  const queue = wallet ? (insuranceQueues?.[ticker] ?? null) : null;
  const pool = pools?.[ticker];
  const price = prices?.[ticker];

  const closeModal = () => {
    refetchBalances();
    setDepositAmount(0n);
    setWithdrawAmount(0n);
    deposit.resetState();
    queueWithdraw.resetState();
    cancelQueue.resetState();
    executeWithdraw.resetState();
    onClose();
  };

  // Show full-screen overlays for in-progress transactions
  if (deposit.isDepositing) {
    return (
      <Dialog modalId={modalId} onClose={() => {}}>
        <LoadingDialogContent title="Depositing" subtitle={`Depositing ${stroopsToDecimalString(depositAmount)} l${ticker}.`} onClick={closeModal} />
      </Dialog>
    );
  }
  if (deposit.isDepositSuccess) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <SuccessDialogContent subtitle={`Successfully deposited ${stroopsToDecimalString(depositAmount)} l${ticker}.`} onClick={closeModal} />
      </Dialog>
    );
  }
  if (deposit.depositError) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <ErrorDialogContent error={deposit.depositError} onClick={closeModal} />
      </Dialog>
    );
  }

  if (queueWithdraw.isQueuing) {
    return (
      <Dialog modalId={modalId} onClose={() => {}}>
        <LoadingDialogContent title="Starting queue" subtitle={`Queueing ${stroopsToDecimalString(withdrawAmount)} l${ticker} for withdrawal.`} onClick={closeModal} />
      </Dialog>
    );
  }
  if (queueWithdraw.isQueueSuccess) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <SuccessDialogContent subtitle={`Withdrawal queue started for ${stroopsToDecimalString(withdrawAmount)} l${ticker}. Come back in 14 days to execute.`} onClick={closeModal} />
      </Dialog>
    );
  }
  if (queueWithdraw.queueError) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <ErrorDialogContent error={queueWithdraw.queueError} onClick={closeModal} />
      </Dialog>
    );
  }

  if (cancelQueue.isCancelling) {
    return (
      <Dialog modalId={modalId} onClose={() => {}}>
        <LoadingDialogContent title="Cancelling queue" subtitle="Cancelling your withdrawal queue." onClick={closeModal} />
      </Dialog>
    );
  }
  if (cancelQueue.isCancelSuccess) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <SuccessDialogContent subtitle="Withdrawal queue cancelled." onClick={closeModal} />
      </Dialog>
    );
  }
  if (cancelQueue.cancelError) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <ErrorDialogContent error={cancelQueue.cancelError} onClick={closeModal} />
      </Dialog>
    );
  }

  if (executeWithdraw.isExecuting) {
    return (
      <Dialog modalId={modalId} onClose={() => {}}>
        <LoadingDialogContent title="Executing withdrawal" subtitle={`Withdrawing your l${ticker} tokens from the insurance pool.`} onClick={closeModal} />
      </Dialog>
    );
  }
  if (executeWithdraw.isExecuteSuccess) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <SuccessDialogContent subtitle={`Successfully withdrew your l${ticker} tokens from the insurance pool.`} onClick={closeModal} />
      </Dialog>
    );
  }
  if (executeWithdraw.executeError) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <ErrorDialogContent error={executeWithdraw.executeError} onClick={closeModal} />
      </Dialog>
    );
  }

  const depositAmountCents = price ? toCents(price, depositAmount) : undefined;
  const withdrawAmountCents = price ? toCents(price, withdrawAmount) : undefined;

  // Compute approximate token value of queued shares at current pool rate
  const queuedTokenValue =
    queue && pool && pool.totalShares > 0n
      ? (queue.queued_shares * pool.totalTokens) / pool.totalShares
      : null;

  const nowSeconds = Date.now() / 1000;
  const isQueueReady = queue
    ? nowSeconds >= Number(queue.queued_at_timestamp) + FOURTEEN_DAYS_IN_SECONDS
    : false;

  return (
    <Dialog className="min-w-[760px]" modalId={modalId} onClose={closeModal}>
      <h3 className="font-bold text-xl mb-4">Manage {name} Insurance</h3>

      <div role="tablist" className="tabs tabs-bordered mb-6">
        <button
          type="button"
          role="tab"
          className={`tab${tab === 'deposit' ? ' tab-active' : ''}`}
          onClick={() => setTab('deposit')}
        >
          Deposit
        </button>
        <button
          type="button"
          role="tab"
          className={`tab${tab === 'withdraw' ? ' tab-active' : ''}`}
          onClick={() => setTab('withdraw')}
        >
          Withdraw
        </button>
      </div>

      {tab === 'deposit' && (
        <>
          <p className="text-sm opacity-70 mb-8">Deposit your l{ticker} tokens to provide insurance coverage for the {name} pool.</p>
          <p className="text-lg mb-2">Amount to deposit</p>
          <CryptoAmountSelector
            max={lTokenBalance}
            value={depositAmount}
            valueCents={depositAmountCents}
            ticker={ticker}
            onChange={setDepositAmount}
            onSelectMaximum={() => setDepositAmount(lTokenBalance)}
          />
          <div className="flex flex-row justify-end mt-8">
            <Button onClick={closeModal} variant="ghost" className="mr-4">Cancel</Button>
            <Button
              disabled={depositAmount === 0n || depositAmount > lTokenBalance}
              onClick={() => deposit.sendTransaction(depositAmount)}
            >
              Deposit
            </Button>
          </div>
        </>
      )}

      {tab === 'withdraw' && !queue && (
        <>
          <p className="text-sm opacity-70 mb-8">
            Queue a withdrawal from the {name} insurance pool. After a 14-day waiting period you can execute the withdrawal.
            During this time you continue to absorb any bad debt events.
          </p>
          <p className="text-lg mb-2">Amount to withdraw</p>
          <CryptoAmountSelector
            max={insuranceBalance}
            value={withdrawAmount}
            valueCents={withdrawAmountCents}
            ticker={ticker}
            onChange={setWithdrawAmount}
            onSelectMaximum={() => setWithdrawAmount(insuranceBalance)}
          />
          <div className="flex flex-row justify-end mt-8">
            <Button onClick={closeModal} variant="ghost" className="mr-4">Cancel</Button>
            <Button
              disabled={withdrawAmount === 0n || withdrawAmount > insuranceBalance}
              onClick={() => queueWithdraw.sendTransaction(withdrawAmount)}
            >
              Start Queue
            </Button>
          </div>
        </>
      )}

      {tab === 'withdraw' && queue && (
        <>
          <p className="text-sm opacity-70 mb-6">Your withdrawal is queued. You continue to absorb bad debt events until you execute the withdrawal.</p>
          <div className="mb-6">
            <QueueProgressBar queuedAtTimestamp={queue.queued_at_timestamp} />
          </div>
          {!isNil(queuedTokenValue) && (
            <div className="mb-6">
              <p className="text-sm opacity-70 mb-1">Queued amount (current value)</p>
              <p className="font-semibold text-lg">{formatAmount(queuedTokenValue)} l{ticker}</p>
              {!isNil(price) && <p className="text-sm opacity-70">{/* dollar value shown via formatAmount */}</p>}
            </div>
          )}
          <div className="flex flex-row justify-end gap-3 mt-8">
            <Button onClick={closeModal} variant="ghost">Cancel</Button>
            <Button onClick={() => cancelQueue.sendTransaction()} variant="ghost">Cancel Queue</Button>
            <Button
              disabled={!isQueueReady}
              onClick={() => executeWithdraw.sendTransaction()}
            >
              {isQueueReady ? 'Execute Withdraw' : 'Not ready yet'}
            </Button>
          </div>
        </>
      )}
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
    if (!wallet || !currency) return;
    setIsDepositing(true);
    try {
      const tx = await currency.contractClient.deposit({ user: wallet.address, amount: stroops });
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

const useQueueWithdrawTransaction = (currency: CurrencyBinding | null) => {
  const { wallet, signTransaction } = useWallet();
  const [isQueuing, setIsQueuing] = useState(false);
  const [isQueueSuccess, setIsQueueSuccess] = useState(false);
  const [queueError, setQueueError] = useState<Error | null>(null);

  const resetState = () => {
    setIsQueuing(false);
    setIsQueueSuccess(false);
    setQueueError(null);
  };

  const sendTransaction = async (amountInTokens: bigint) => {
    if (!wallet || !currency) return;
    setIsQueuing(true);
    try {
      const tx = await currency.contractClient.queue_withdraw({ user: wallet.address, amount_in_tokens: amountInTokens });
      await tx.signAndSend({ signTransaction });
      setIsQueueSuccess(true);
      setQueueError(null);
    } catch (err) {
      setQueueError(err as Error);
      setIsQueueSuccess(false);
    }
    setIsQueuing(false);
  };

  return { isQueuing, isQueueSuccess, queueError, sendTransaction, resetState };
};

const useCancelQueueTransaction = (currency: CurrencyBinding | null) => {
  const { wallet, signTransaction } = useWallet();
  const [isCancelling, setIsCancelling] = useState(false);
  const [isCancelSuccess, setIsCancelSuccess] = useState(false);
  const [cancelError, setCancelError] = useState<Error | null>(null);

  const resetState = () => {
    setIsCancelling(false);
    setIsCancelSuccess(false);
    setCancelError(null);
  };

  const sendTransaction = async () => {
    if (!wallet || !currency) return;
    setIsCancelling(true);
    try {
      const tx = await currency.contractClient.cancel_queue({ user: wallet.address });
      await tx.signAndSend({ signTransaction });
      setIsCancelSuccess(true);
      setCancelError(null);
    } catch (err) {
      setCancelError(err as Error);
      setIsCancelSuccess(false);
    }
    setIsCancelling(false);
  };

  return { isCancelling, isCancelSuccess, cancelError, sendTransaction, resetState };
};

const useExecuteWithdrawTransaction = (currency: CurrencyBinding | null) => {
  const { wallet, signTransaction } = useWallet();
  const [isExecuting, setIsExecuting] = useState(false);
  const [isExecuteSuccess, setIsExecuteSuccess] = useState(false);
  const [executeError, setExecuteError] = useState<Error | null>(null);

  const resetState = () => {
    setIsExecuting(false);
    setIsExecuteSuccess(false);
    setExecuteError(null);
  };

  const sendTransaction = async () => {
    if (!wallet || !currency) return;
    setIsExecuting(true);
    try {
      const tx = await currency.contractClient.execute_withdraw({ user: wallet.address });
      await tx.signAndSend({ signTransaction });
      setIsExecuteSuccess(true);
      setExecuteError(null);
    } catch (err) {
      setExecuteError(err as Error);
      setIsExecuteSuccess(false);
    }
    setIsExecuting(false);
  };

  return { isExecuting, isExecuteSuccess, executeError, sendTransaction, resetState };
};
