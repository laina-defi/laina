import { useState } from 'react';

import { Button } from '@components/Button';
import { Dialog, ErrorDialogContent, LoadingDialogContent, SuccessDialogContent } from '@components/Dialog';
import { type AuctionWithDetails, useAuctions } from '@contexts/auctions-context';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import { stroopsToDecimalString } from '@lib/converters';
import { formatCentAmount, toCents } from '@lib/formatting';

export interface ClaimModalProps {
  modalId: string;
  onClose: VoidFunction;
  auction: AuctionWithDetails | null;
  currentLedger: number;
}

const AUCTION_DURATION = 17_280;

export const ClaimModal = ({ modalId, onClose, auction, currentLedger }: ClaimModalProps) => {
  const { refetchAuctions } = useAuctions();
  const { prices, refetchPools } = usePools();
  const { isClaiming, isClaimSuccess, claimError, resetState, sendClaim } = useClaimTransaction();
  const [wasLikelyHealthyAtClaim, setWasLikelyHealthyAtClaim] = useState(false);

  if (!auction) return <Dialog modalId={modalId} onClose={onClose} />;

  const { auctionItem, totalDebt, borrowedTicker, collateralAmount, collateralTicker } = auction;

  const elapsed = Math.min(1, Math.max(0, (currentLedger - auctionItem.start_ledger) / AUCTION_DURATION));
  const paymentNeeded = BigInt(Math.floor(Number(totalDebt) * (1 - elapsed)));
  // 1% buffer so the transaction doesn't fail if a bit more interest accrues before execution
  const paymentWithBuffer = (paymentNeeded * 101n) / 100n;
  const writeoffAmount = totalDebt - paymentNeeded;

  const borrowedPrice = prices?.[borrowedTicker];
  const collateralPrice = prices?.[collateralTicker];
  const totalDebtUsd = borrowedPrice ? toCents(borrowedPrice, totalDebt) : null;
  const collateralUsd = collateralPrice ? toCents(collateralPrice, collateralAmount) : null;
  const isLikelyHealthyAgain = totalDebtUsd !== null && collateralUsd !== null && collateralUsd > totalDebtUsd;

  const closeModal = () => {
    resetState();
    refetchAuctions();
    refetchPools();
    onClose();
  };

  if (isClaiming) {
    return (
      <Dialog modalId={modalId} onClose={() => {}}>
        <LoadingDialogContent
          title={wasLikelyHealthyAtClaim ? 'Cancelling auction' : 'Claiming auction'}
          subtitle={
            wasLikelyHealthyAtClaim
              ? 'Checking current health — the auction will be cancelled if the loan has recovered.'
              : `Paying ${stroopsToDecimalString(paymentWithBuffer)} ${borrowedTicker} to claim collateral.`
          }
          onClick={closeModal}
        />
      </Dialog>
    );
  }

  if (isClaimSuccess) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <SuccessDialogContent
          title={wasLikelyHealthyAtClaim ? 'Auction cancelled' : 'Auction claimed!'}
          subtitle={
            wasLikelyHealthyAtClaim
              ? 'The loan has recovered — the auction was cancelled. No payment was taken and no collateral was transferred.'
              : `You received ${stroopsToDecimalString(collateralAmount)} ${collateralTicker}.`
          }
          onClick={closeModal}
        />
      </Dialog>
    );
  }

  if (claimError) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <ErrorDialogContent error={claimError} onClick={closeModal} />
      </Dialog>
    );
  }

  return (
    <Dialog modalId={modalId} onClose={closeModal} className="overflow-x-hidden">
      <ClaimConfirmContent
        paymentWithBuffer={paymentWithBuffer}
        borrowedTicker={borrowedTicker}
        collateralAmount={collateralAmount}
        collateralTicker={collateralTicker}
        writeoffAmount={writeoffAmount}
        elapsed={elapsed}
        currentLedger={currentLedger}
        auctionEndLedger={auctionItem.end_ledger}
        auction={auction}
        isLikelyHealthyAgain={isLikelyHealthyAgain}
        onCancel={closeModal}
        onClaim={() => {
          setWasLikelyHealthyAtClaim(isLikelyHealthyAgain);
          sendClaim({
            loan_id: auctionItem.loan_id,
            amount: paymentWithBuffer,
          });
        }}
      />
    </Dialog>
  );
};

interface ClaimConfirmContentProps {
  paymentWithBuffer: bigint;
  borrowedTicker: string;
  collateralAmount: bigint;
  collateralTicker: string;
  writeoffAmount: bigint;
  elapsed: number;
  currentLedger: number;
  auctionEndLedger: number;
  auction: AuctionWithDetails;
  isLikelyHealthyAgain: boolean;
  onCancel: VoidFunction;
  onClaim: VoidFunction;
}

const ClaimConfirmContent = ({
  paymentWithBuffer,
  borrowedTicker,
  collateralAmount,
  collateralTicker,
  writeoffAmount,
  elapsed,
  currentLedger,
  auctionEndLedger,
  auction,
  isLikelyHealthyAgain,
  onCancel,
  onClaim,
}: ClaimConfirmContentProps) => {
  const { prices } = usePools();
  const { wallet, walletBalances } = useWallet();

  const borrowedPrice = prices?.[auction.borrowedTicker];
  const collateralPrice = prices?.[auction.collateralTicker];

  // Use paymentNeeded (without buffer) for display and P/L, matching the auction card values.
  // paymentWithBuffer is only used for the transaction to avoid failure from accrued interest.
  const paymentNeeded = BigInt(Math.floor(Number(auction.totalDebt) * (1 - elapsed)));

  const paymentUsd = borrowedPrice ? toCents(borrowedPrice, paymentNeeded) : null;
  const collateralUsd = collateralPrice ? toCents(collateralPrice, collateralAmount) : null;

  const plUsd = paymentUsd !== null && collateralUsd !== null ? collateralUsd - paymentUsd : null;
  const plPct =
    plUsd !== null && paymentUsd !== null && paymentUsd > 0n
      ? (Number(plUsd) / Number(paymentUsd)) * 100
      : paymentUsd === 0n
        ? Number.POSITIVE_INFINITY
        : null;

  const ledgersRemaining = Math.max(0, auctionEndLedger - currentLedger);
  const secondsRemaining = ledgersRemaining * 5;
  const hoursRemaining = (secondsRemaining / 3600).toFixed(1);

  const borrowerBalance = walletBalances?.[auction.borrowedTicker];
  const hasInsufficientBalance =
    wallet &&
    borrowerBalance?.trustLine &&
    paymentWithBuffer > BigInt(borrowerBalance.balanceLine.balance.replace('.', ''));

  return (
    <div className="w-[480px] max-w-full">
      <h3 className="font-bold text-xl mb-6">Claim Bad Debt Auction</h3>

      {isLikelyHealthyAgain && (
        <div className="alert alert-warning mb-6 text-sm">
          <span>
            This loan's collateral value appears to have recovered above the total debt. If so, claiming will simply
            cancel the auction — no payment will be taken from you and no collateral will be transferred.
          </span>
        </div>
      )}

      {!isLikelyHealthyAgain && (
        <div className="rounded border border-grey-light mb-6">
          <div className="p-4">
            <p className="text-sm text-grey mb-1">You pay</p>
            <p className="text-xl font-semibold">
              {roundToSigFigs(paymentNeeded)} {borrowedTicker}
            </p>
            {paymentUsd !== null && <p className="text-sm text-grey">{formatCentAmount(paymentUsd)}</p>}
            <p className="text-xs text-grey mt-1">
              A 1% buffer is added to the transaction to cover accrued interest; excess is refunded.
            </p>
            {hasInsufficientBalance && (
              <p className="text-xs text-red mt-2">Insufficient {borrowedTicker} balance to cover this claim.</p>
            )}
          </div>

          <div className="border-t border-grey-light p-4">
            <p className="text-sm text-grey mb-1">You receive</p>
            <p className="text-xl font-semibold">
              {roundToSigFigs(collateralAmount)} {collateralTicker}
            </p>
            {collateralUsd !== null && <p className="text-sm text-grey">{formatCentAmount(collateralUsd)}</p>}
          </div>

          {plUsd !== null && (
            <div className="border-t border-grey-light p-4">
              <p className="text-sm text-grey mb-1">Net P/L</p>
              <p className={`text-xl font-semibold ${plUsd >= 0n ? 'text-success' : 'text-error'}`}>
                {plUsd >= 0n ? '+' : ''}
                {formatCentAmount(plUsd)}
                {plPct !== null && Number.isFinite(plPct) && (
                  <span className="text-lg ml-2">
                    ({plPct >= 0 ? '+' : ''}
                    {plPct.toFixed(1)}%)
                  </span>
                )}
                {plPct === Number.POSITIVE_INFINITY && <span className="text-lg ml-2">(free!)</span>}
              </p>
            </div>
          )}

          {writeoffAmount > 0n && (
            <div className="border-t border-grey-light p-4">
              <p className="text-sm text-grey mb-1">Insurance pool absorbs</p>
              <p className="text-xl font-semibold">
                {roundToSigFigs(writeoffAmount)} {borrowedTicker}
              </p>
              <p className="text-xs text-grey mt-1">This amount of bad debt is written off from the insurance pool.</p>
            </div>
          )}
        </div>
      )}

      <div className="mb-6">
        <AuctionProgressBar elapsed={elapsed} hoursRemaining={ledgersRemaining > 0 ? hoursRemaining : null} />
      </div>

      <div className="flex justify-end gap-3">
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button onClick={onClaim} disabled={!isLikelyHealthyAgain && (!!hasInsufficientBalance || !wallet)}>
          {!wallet ? 'Connect wallet first' : isLikelyHealthyAgain ? 'Confirm Cancellation' : 'Claim Auction'}
        </Button>
      </div>
    </div>
  );
};

const AuctionProgressBar = ({ elapsed, hoursRemaining }: { elapsed: number; hoursRemaining: string | null }) => {
  const pct = elapsed * 100;
  let status: string;
  if (elapsed < 0.25) {
    status = 'Auction has just started — likely not profitable yet';
  } else if (elapsed < 0.5) {
    status = 'Discount is growing — may become worthwhile soon';
  } else if (elapsed < 0.75) {
    status = 'Significant discount applied — but others may claim first';
  } else {
    status = 'Deep discount — attractive, but competition is high';
  }

  return (
    <>
      <div className="w-full h-3 bg-grey rounded-full overflow-hidden mb-2">
        <div
          className="h-full bg-gradient-to-r from-cyan to-magenta rounded-full transition-all"
          style={{ width: `${pct}%` }}
        />
      </div>
      <div className="flex justify-between text-sm text-grey mb-1">
        <span>Auction progress · {pct.toFixed(1)}%</span>
        {hoursRemaining !== null && <span>{hoursRemaining}h remaining</span>}
      </div>
      <p className="text-xs text-grey">{status}</p>
    </>
  );
};

const roundToSigFigs = (stroops: bigint, sigFigs = 7): string => {
  const num = Number(stroops) / 10_000_000;
  if (num === 0) return '0';
  const magnitude = Math.floor(Math.log10(Math.abs(num)));
  const decimalPlaces = Math.max(0, sigFigs - 1 - magnitude);
  const fixed = num.toFixed(decimalPlaces);
  return decimalPlaces > 0 ? fixed.replace(/\.?0+$/, '') : fixed;
};

const useClaimTransaction = () => {
  const { wallet, signTransaction } = useWallet();
  const [isClaiming, setIsClaiming] = useState(false);
  const [isClaimSuccess, setIsClaimSuccess] = useState(false);
  const [claimError, setClaimError] = useState<Error | null>(null);

  const resetState = () => {
    setIsClaiming(false);
    setIsClaimSuccess(false);
    setClaimError(null);
  };

  const sendClaim = async ({
    loan_id,
    amount,
  }: {
    loan_id: { borrower_address: string; nonce: bigint };
    amount: bigint;
  }) => {
    if (!wallet) {
      alert('Please connect your wallet first!');
      return;
    }

    setIsClaiming(true);

    try {
      const tx = await loanManagerClient.claim_bad_debt_auction({
        user: wallet.address,
        loan_id,
        amount,
      });
      await tx.signAndSend({ signTransaction });
      setIsClaimSuccess(true);
      setClaimError(null);
    } catch (err) {
      setClaimError(err as Error);
      setIsClaimSuccess(false);
    }

    setIsClaiming(false);
  };

  return { isClaiming, isClaimSuccess, claimError, resetState, sendClaim };
};
