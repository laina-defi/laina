import { useMemo } from 'react';

import { Button } from '@components/Button';
import { StellarExpertLink } from '@components/Link';
import { Loading } from '@components/Loading';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { isBalanceZero } from '@lib/converters';
import { calcBorrowerLaiAPR, formatAmount, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
import { PiInfo } from 'react-icons/pi';
import type { CurrencyBinding } from 'src/currency-bindings';

export interface BorrowableAssetProps {
  currency: CurrencyBinding;
  onBorrowClicked: VoidFunction;
}

export const BorrowableAsset = ({ currency, onBorrowClicked }: BorrowableAssetProps) => {
  const { icon, name, ticker, issuerName, contractId } = currency;

  const { wallet, walletBalances } = useWallet();
  const { prices, pools } = usePools();
  const price = prices?.[ticker];
  const pool = pools?.[ticker];

  // Does the user have some other token in their wallet to use as collateral?
  const isCollateral = !walletBalances
    ? false
    : Object.entries(walletBalances)
        .filter(([t]) => t !== ticker)
        .some(([, b]) => b.trustLine && !isBalanceZero(b.balanceLine.balance));

  const borrowDisabled = !wallet || !isCollateral || !pool || pool.availableBalanceTokens === 0n;

  const tooltip = useMemo(() => {
    if (!pool) return 'The pool is loading';
    if (pool.availableBalanceTokens === 0n) return 'The pool has no assets to borrow';
    if (!wallet) return 'Connect a wallet first';
    if (!isCollateral) return 'Another token needed for collateral';
    return 'Something odd happened.';
  }, [pool, wallet, isCollateral]);

  const aprContent =
    pool && price !== undefined ? (() => {
      const baseAPR = Number(pool.annualInterestRate) / 100_000;
      const laiAPR = calcBorrowerLaiAPR(pool.totalBalanceTokens, pool.availableBalanceTokens, price);
      const netAPR = baseAPR - laiAPR;
      const tip = `Base borrow rate: ${baseAPR.toFixed(2)}%. You earn LAI token rewards worth ${laiAPR.toFixed(2)}% of your borrowed amount per year, reducing your net cost to ${netAPR.toFixed(2)}%.`;
      return (
        <span className="flex items-center gap-1">
          <span className="font-semibold leading-5">{netAPR.toFixed(2)} %</span>
          <span className="tooltip tooltip-left cursor-help" data-tip={tip}>
            <PiInfo size={14} className="opacity-40 hover:opacity-70 transition-opacity" />
          </span>
        </span>
      );
    })() : (
      <Loading size="xs" />
    );

  const actionButton = borrowDisabled ? (
    <div className="tooltip w-full md:w-auto" data-tip={tooltip}>
      <Button disabled={true} onClick={() => {}} className="w-full md:w-auto">
        Borrow
      </Button>
    </div>
  ) : (
    <Button onClick={onBorrowClicked} className="w-full md:w-auto">
      Borrow
    </Button>
  );

  return (
    <div className="border-b border-grey-light last:border-none hover:bg-grey-lighter/30 transition-colors">
      {/* ── Mobile card layout ── */}
      <div className="md:hidden py-4">
        <div className="flex items-center gap-3 mb-4">
          <img src={icon} alt="" className="w-12 h-12 flex-none" />
          <div className="flex-1 min-w-0">
            <h2 className="font-semibold text-base tracking-tight truncate">{name}</h2>
            <p className="text-xs text-grey">
              {ticker} · {issuerName}
            </p>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3 mb-4">
          <div>
            <p className="text-xs text-grey mb-0.5">Available</p>
            <p className="text-sm font-semibold leading-5">
              {pool ? formatAmount(pool.availableBalanceTokens) : <Loading size="xs" />}
            </p>
            <p className="text-xs text-grey">
              {!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.availableBalanceTokens)}
            </p>
          </div>
          <div>
            <p className="text-xs text-grey mb-0.5">Borrow APR</p>
            <div className="text-sm">{aprContent}</div>
          </div>
        </div>

        <StellarExpertLink contractId={contractId} text="View pool contract" className="text-sm mb-3" />

        {actionButton}
      </div>

      {/* ── Desktop grid row layout ── */}
      <div className="hidden md:grid md:grid-cols-[80px_1fr_90px_150px_150px_130px] md:items-center md:min-h-[6.5rem] md:px-1">
        {/* Icon */}
        <div className="flex justify-center">
          <img src={icon} alt="" className="max-h-12 max-w-[48px]" />
        </div>

        {/* Name + link */}
        <div>
          <h2 className="font-semibold text-xl tracking-tight">{name}</h2>
          <StellarExpertLink contractId={contractId} text="View pool contract" />
        </div>

        {/* Ticker */}
        <div>
          <p className="font-semibold leading-5">{ticker}</p>
          <p className="text-sm text-grey">{issuerName}</p>
        </div>

        {/* Available */}
        <div>
          <p className="text-lg font-semibold leading-5">
            {pool ? formatAmount(pool.availableBalanceTokens) : <Loading size="xs" />}
          </p>
          <p className="text-sm text-grey">
            {!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.availableBalanceTokens)}
          </p>
        </div>

        {/* Borrow APR */}
        <div className="text-lg">{aprContent}</div>

        {/* Action */}
        <div className="flex justify-end pr-2 py-2">{actionButton}</div>
      </div>
    </div>
  );
};
