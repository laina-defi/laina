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
  const { refetchPools } = usePools();
  const { isClaiming, isClaimSuccess, claimError, resetState, sendClaim } = useClaimTransaction();

  if (!auction) return <Dialog modalId={modalId} onClose={onClose} />;

  const { auctionItem, totalDebt, borrowedTicker, collateralAmount, collateralTicker, borrowerAddress } = auction;

  const elapsed = Math.min(1, Math.max(0, (currentLedger - auctionItem.start_ledger) / AUCTION_DURATION));
  const paymentNeeded = BigInt(Math.floor(Number(totalDebt) * (1 - elapsed)));
  // 1% buffer so the transaction doesn't fail if a bit more interest accrues before execution
  const paymentWithBuffer = (paymentNeeded * 101n) / 100n;
  const writeoffAmount = totalDebt - paymentNeeded;

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
          title="Claiming auction"
          subtitle={`Paying ${stroopsToDecimalString(paymentWithBuffer)} ${borrowedTicker} to claim collateral.`}
          onClick={closeModal}
        />
      </Dialog>
    );
  }

  if (isClaimSuccess) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <SuccessDialogContent
          title="Auction claimed!"
          subtitle={`You received ${stroopsToDecimalString(collateralAmount)} ${collateralTicker}.`}
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
        borrowerAddress={borrowerAddress}
        paymentWithBuffer={paymentWithBuffer}
        borrowedTicker={borrowedTicker}
        collateralAmount={collateralAmount}
        collateralTicker={collateralTicker}
        writeoffAmount={writeoffAmount}
        elapsed={elapsed}
        currentLedger={currentLedger}
        auctionEndLedger={auctionItem.end_ledger}
        auction={auction}
        onCancel={closeModal}
        onClaim={() =>
          sendClaim({
            loan_id: auctionItem.loan_id,
            amount: paymentWithBuffer,
          })
        }
      />
    </Dialog>
  );
};

interface ClaimConfirmContentProps {
  borrowerAddress: string;
  paymentWithBuffer: bigint;
  borrowedTicker: string;
  collateralAmount: bigint;
  collateralTicker: string;
  writeoffAmount: bigint;
  elapsed: number;
  currentLedger: number;
  auctionEndLedger: number;
  auction: AuctionWithDetails;
  onCancel: VoidFunction;
  onClaim: VoidFunction;
}

const ClaimConfirmContent = ({
  borrowerAddress,
  paymentWithBuffer,
  borrowedTicker,
  collateralAmount,
  collateralTicker,
  writeoffAmount,
  elapsed,
  currentLedger,
  auctionEndLedger,
  auction,
  onCancel,
  onClaim,
}: ClaimConfirmContentProps) => {
  const { prices } = usePools();
  const { wallet, walletBalances } = useWallet();

  const borrowedPrice = prices?.[auction.borrowedTicker];
  const collateralPrice = prices?.[auction.collateralTicker];

  const paymentUsd = borrowedPrice ? toCents(borrowedPrice, paymentWithBuffer) : null;
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

  const shortAddress = `${borrowerAddress.slice(0, 6)}...${borrowerAddress.slice(-4)}`;

  return (
    <div className="w-[480px] max-w-full">
      <h3 className="font-bold text-xl mb-6">Claim Bad Debt Auction</h3>

      <div className="rounded p-[1px] bg-gradient-to-br from-cyan to-magenta shadow mb-6">
        <div className="rounded bg-black text-white p-4 space-y-3">
          <Row label="Borrower">
            <span className="font-mono text-sm tooltip" data-tip={borrowerAddress}>
              {shortAddress}
            </span>
          </Row>
          <Row label="Auction progress">
            <div className="flex items-center gap-2">
              <div className="w-24 h-1.5 bg-grey-dark rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-cyan to-magenta rounded-full transition-all"
                  style={{ width: `${elapsed * 100}%` }}
                />
              </div>
              <span className="text-sm text-grey">{(elapsed * 100).toFixed(1)}%</span>
            </div>
          </Row>
          {ledgersRemaining > 0 && (
            <Row label="Time remaining">
              <span className="text-sm">{hoursRemaining}h</span>
            </Row>
          )}
        </div>
      </div>

      <div className="rounded shadow border-2 border-grey-light bg-white mb-6">
        <div className="p-4">
          <p className="text-sm text-grey mb-1">You pay (max)</p>
          <p className="text-xl font-semibold">
            {roundToSigFigs(paymentWithBuffer)} {borrowedTicker}
          </p>
          {paymentUsd !== null && <p className="text-sm text-grey">{formatCentAmount(paymentUsd)}</p>}
          <p className="text-xs text-grey mt-1">Excess above actual cost is refunded automatically.</p>
        </div>

        <div className="border-t-2 border-grey-light p-4">
          <p className="text-sm text-grey mb-1">You receive</p>
          <p className="text-xl font-semibold">
            {roundToSigFigs(collateralAmount)} {collateralTicker}
          </p>
          {collateralUsd !== null && <p className="text-sm text-grey">{formatCentAmount(collateralUsd)}</p>}
        </div>

        {plUsd !== null && (
          <div className="border-t-2 border-grey-light p-4">
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
          <div className="border-t-2 border-grey-light p-4">
            <p className="text-sm text-grey mb-1">Insurance pool absorbs</p>
            <p className="text-xl font-semibold">
              {roundToSigFigs(writeoffAmount)} {borrowedTicker}
            </p>
            <p className="text-xs text-grey mt-1">This amount of bad debt is written off from the insurance pool.</p>
          </div>
        )}
      </div>

      <div className="flex justify-end gap-3">
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <div className={hasInsufficientBalance ? 'tooltip' : ''} data-tip={hasInsufficientBalance ? `Insufficient ${borrowedTicker} balance to cover this claim.` : undefined}>
          <Button onClick={onClaim} disabled={!!hasInsufficientBalance || !wallet}>
            {!wallet ? 'Connect wallet first' : 'Claim Auction'}
          </Button>
        </div>
      </div>
    </div>
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

const Row = ({ label, children }: { label: string; children: React.ReactNode }) => (
  <div className="flex items-center justify-between">
    <span className="text-sm text-grey">{label}</span>
    <span className="text-sm font-semibold">{children}</span>
  </div>
);

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
