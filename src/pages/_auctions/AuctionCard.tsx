import { useState } from 'react';

import { Button } from '@components/Button';
import Identicon from '@components/Identicon';
import { Loading } from '@components/Loading';
import type { AuctionWithDetails } from '@contexts/auctions-context';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { stroopsToDecimalString } from '@lib/converters';
import { formatAmount, formatCentAmount, toCents, toDollarsFormatted } from '@lib/formatting';
import { PiArrowSquareOut, PiCaretDown } from 'react-icons/pi';

export interface AuctionCardProps {
  auction: AuctionWithDetails;
  currentLedger: number;
  onClaimClicked: VoidFunction;
}

const AUCTION_DURATION = 17_280;

export const AuctionCard = ({ auction, currentLedger, onClaimClicked }: AuctionCardProps) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const { prices } = usePools();
  const { wallet } = useWallet();

  const {
    auctionItem,
    totalDebt,
    borrowedTicker,
    collateralAmount,
    collateralTicker,
    borrowerAddress,
    unpaidInterest,
  } = auction;

  const elapsed = Math.min(1, Math.max(0, (currentLedger - auctionItem.start_ledger) / AUCTION_DURATION));
  const paymentNeeded = BigInt(Math.floor(Number(totalDebt) * (1 - elapsed)));
  const writeoffAmount = totalDebt - paymentNeeded;

  const borrowedPrice = prices?.[borrowedTicker];
  const collateralPrice = prices?.[collateralTicker];

  const paymentUsd = borrowedPrice ? toCents(borrowedPrice, paymentNeeded) : null;
  const collateralUsd = collateralPrice ? toCents(collateralPrice, collateralAmount) : null;

  const plUsd = paymentUsd !== null && collateralUsd !== null ? collateralUsd - paymentUsd : null;
  const plPct =
    plUsd !== null && paymentUsd !== null && paymentUsd > 0n
      ? (Number(plUsd) / Number(paymentUsd)) * 100
      : paymentUsd === 0n
        ? Number.POSITIVE_INFINITY
        : null;

  const ledgersRemaining = Math.max(0, auctionItem.end_ledger - currentLedger);
  const hoursRemaining = ((ledgersRemaining * 5) / 3600).toFixed(1);
  const isExpired = currentLedger >= auctionItem.end_ledger;

  const shortBorrower = `${borrowerAddress.slice(0, 6)}...${borrowerAddress.slice(-4)}`;
  const stellarExpertNetwork = import.meta.env.PUBLIC_STELLAR_NETWORK === 'mainnet' ? 'public' : 'testnet';
  const borrowerExplorerUrl = `https://stellar.expert/explorer/${stellarExpertNetwork}/account/${borrowerAddress}`;

  const plColor = plUsd === null ? '' : plUsd >= 0n ? 'text-success' : 'text-error';
  const plLabel =
    plUsd === null
      ? null
      : plPct === Number.POSITIVE_INFINITY
        ? `${formatCentAmount(plUsd)} (free!)`
        : `${plUsd >= 0n ? '+' : ''}${formatCentAmount(plUsd)}${plPct !== null ? ` (${plPct >= 0 ? '+' : ''}${plPct.toFixed(1)}%)` : ''}`;

  const claimButton = wallet ? (
    <Button onClick={onClaimClicked} className="w-full md:w-auto">
      Claim
    </Button>
  ) : (
    <div className="tooltip w-full md:w-auto" data-tip="Connect a wallet first">
      <Button disabled onClick={() => {}} className="w-full md:w-auto">
        Claim
      </Button>
    </div>
  );

  const chevron = (
    <button
      type="button"
      onClick={() => setIsExpanded((v) => !v)}
      className="p-1 rounded flex items-center justify-center hover:bg-grey-lighter/50 transition-colors text-grey"
      aria-label={isExpanded ? 'Collapse details' : 'Expand details'}
    >
      <span className={`inline-block transition-transform duration-300 ${isExpanded ? 'rotate-180' : ''}`}>
        <PiCaretDown size={18} />
      </span>
    </button>
  );

  const progressBar = (
    <div className="mt-1 w-full h-1 bg-grey-light rounded-full overflow-hidden">
      <div
        className={`h-full rounded-full transition-all ${plUsd !== null && plUsd >= 0n ? 'bg-success' : 'bg-gradient-to-r from-cyan to-magenta'}`}
        style={{ width: `${elapsed * 100}%` }}
      />
    </div>
  );

  const expandedPanel = (
    <div
      className={`overflow-hidden transition-[max-height] duration-300 ease-in-out ${isExpanded ? 'max-h-60' : 'max-h-0'}`}
    >
      <div className="px-4 pb-4 pt-2 grid grid-cols-2 md:grid-cols-3 gap-4 bg-grey-lighter/20 border-t border-grey-light">
        <div>
          <p className="text-xs text-grey mb-0.5">Unpaid interest</p>
          <p className="text-sm font-semibold">
            {stroopsToDecimalString(unpaidInterest)} {borrowedTicker}
          </p>
        </div>
        <div>
          <p className="text-xs text-grey mb-0.5">Insurance absorbs</p>
          <p className="text-sm font-semibold">
            {writeoffAmount > 0n ? `${stroopsToDecimalString(writeoffAmount)} ${borrowedTicker}` : '—'}
          </p>
        </div>
        <div>
          <p className="text-xs text-grey mb-1">Auction progress</p>
          {progressBar}
          <p className="text-xs text-grey mt-1">
            {isExpired ? 'Expired — pay 0' : `${(elapsed * 100).toFixed(0)}% elapsed · ${hoursRemaining}h left`}
          </p>
        </div>
      </div>
    </div>
  );

  return (
    <div className="border-b border-grey-light last:border-none hover:bg-grey-lighter/30 transition-colors">
      {/* Mobile layout */}
      <div className="md:hidden py-4 px-1">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Identicon address={borrowerAddress} size="sm" />
            <div>
              <p className="text-xs text-grey mb-0.5">Borrower</p>
              <a
                href={borrowerExplorerUrl}
                target="_blank"
                rel="noreferrer"
                className="font-mono text-sm underline inline-flex flex-row items-center gap-0.5 hover:text-grey transition cursor-pointer"
              >
                {shortBorrower}
                <PiArrowSquareOut size={13} />
              </a>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {isExpired && <span className="badge badge-success badge-sm">Expired — free!</span>}
            {chevron}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3 mb-3">
          <div>
            <p className="text-xs text-grey mb-0.5">Total debt</p>
            <p className="text-sm font-semibold">
              {formatAmount(totalDebt)} {borrowedTicker}
            </p>
            {borrowedPrice && <p className="text-xs text-grey">{toDollarsFormatted(borrowedPrice, totalDebt)}</p>}
          </div>
          <div>
            <p className="text-xs text-grey mb-0.5">Collateral</p>
            <p className="text-sm font-semibold">
              {formatAmount(collateralAmount)} {collateralTicker}
            </p>
            {collateralPrice && (
              <p className="text-xs text-grey">{toDollarsFormatted(collateralPrice, collateralAmount)}</p>
            )}
          </div>
          <div>
            <p className="text-xs text-grey mb-0.5">Pay now</p>
            <p className="text-sm font-semibold">
              {formatAmount(paymentNeeded)} {borrowedTicker}
            </p>
            {paymentUsd !== null && <p className="text-xs text-grey">{formatCentAmount(paymentUsd)}</p>}
          </div>
          <div>
            <p className="text-xs text-grey mb-0.5">P/L now</p>
            {plLabel ? <p className={`text-sm font-semibold ${plColor}`}>{plLabel}</p> : <Loading size="xs" />}
          </div>
        </div>

        {claimButton}
      </div>

      {/* Desktop grid row */}
      <div className="hidden md:grid md:grid-cols-[200px_1fr_1fr_1fr_160px_130px_40px] md:items-center md:min-h-[5.5rem] md:px-1">
        {/* Borrower */}
        <div className="flex items-center gap-2">
          <Identicon address={borrowerAddress} size="sm" />
          <div>
            <a
              href={borrowerExplorerUrl}
              target="_blank"
              rel="noreferrer"
              className="font-mono text-sm underline inline-flex flex-row items-center gap-0.5 hover:text-grey transition cursor-pointer"
            >
              {shortBorrower}
              <PiArrowSquareOut size={13} />
            </a>
            {isExpired && <div className="mt-1"><span className="badge badge-success badge-sm">Expired — free!</span></div>}
          </div>
        </div>

        {/* Total debt */}
        <div>
          <p className="text-base font-semibold leading-5">
            {formatAmount(totalDebt)} {borrowedTicker}
          </p>
          {borrowedPrice ? (
            <p className="text-sm text-grey">{toDollarsFormatted(borrowedPrice, totalDebt)}</p>
          ) : (
            <Loading size="xs" />
          )}
        </div>

        {/* Collateral */}
        <div>
          <p className="text-base font-semibold leading-5">
            {formatAmount(collateralAmount)} {collateralTicker}
          </p>
          {collateralPrice ? (
            <p className="text-sm text-grey">{toDollarsFormatted(collateralPrice, collateralAmount)}</p>
          ) : (
            <Loading size="xs" />
          )}
        </div>

        {/* Pay now (decaying) */}
        <div>
          <p className="text-base font-semibold leading-5">
            {formatAmount(paymentNeeded)} {borrowedTicker}
          </p>
          {paymentUsd !== null && <p className="text-sm text-grey">{formatCentAmount(paymentUsd)}</p>}
        </div>

        {/* P/L */}
        <div>
          {plLabel ? (
            <p className={`text-base font-semibold leading-5 ${plColor}`}>{plLabel}</p>
          ) : (
            <Loading size="xs" />
          )}
        </div>

        {/* Claim button */}
        <div className="flex justify-end pr-2 py-2">{claimButton}</div>

        {/* Expand toggle */}
        <div className="flex justify-center">{chevron}</div>
      </div>

      {expandedPanel}
    </div>
  );
};
