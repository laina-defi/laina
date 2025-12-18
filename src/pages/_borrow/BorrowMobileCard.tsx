import { useMemo } from 'react';

import { Button } from '@components/Button';
import { Loading } from '@components/Loading';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { isBalanceZero } from '@lib/converters';
import { formatAPR, formatAmount, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
import type { CurrencyBinding } from 'src/currency-bindings';

export interface BorrowMobileCardProps {
  currency: CurrencyBinding;
  onBorrowClicked: VoidFunction;
}

export const BorrowMobileCard = ({ currency, onBorrowClicked }: BorrowMobileCardProps) => {
  const { icon, name, ticker, issuerName } = currency;

  const { wallet, walletBalances } = useWallet();
  const { prices, pools } = usePools();
  const pool = pools?.[ticker];
  const price = prices?.[ticker];

  // Does the user have some other token in their wallet to use as a collateral?
  const isCollateral = !walletBalances
    ? false
    : Object.entries(walletBalances)
        .filter(([t, _b]) => t !== ticker)
        .some(([_t, b]) => b.trustLine && !isBalanceZero(b.balanceLine.balance));

  const borrowDisabled = !wallet || !isCollateral || !pool || pool.availableBalanceTokens === 0n;

  const tooltip = useMemo(() => {
    if (!pool) return 'The pool is loading';
    if (pool.availableBalanceTokens === 0n) return 'the pool has no assets to borrow';
    if (!wallet) return 'Connect a wallet first';
    if (!isCollateral) return 'Another token needed for the collateral';
    return 'Something odd happened.';
  }, [pool, wallet, isCollateral]);

  return (
    <div className="border-b-2 border-base-300 p-4 mb-6 bg-base-100">
      <div className="flex items-center gap-3 mb-3">
        <img src={icon} alt="" className="w-12 h-12" />
        <div className="flex-1">
          <h2 className="font-semibold text-xl tracking-tight">{name}</h2>
          <p className="text-sm opacity-70">
            {ticker} • {issuerName}
          </p>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4 mb-3">
        <div>
          <p className="text-sm opacity-70 mb-1">Available</p>
          <p className="font-semibold text-lg">
            {pool ? formatAmount(pool.availableBalanceTokens) : <Loading size="xs" />}
          </p>
          <p className="text-sm opacity-70">
            {!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.availableBalanceTokens)}
          </p>
        </div>
        <div>
          <p className="text-sm opacity-70 mb-1">Borrow APY</p>
          <p className="font-semibold text-lg">{pool ? formatAPR(pool.annualInterestRate) : <Loading size="xs" />}</p>
        </div>
      </div>

      <div className="flex flex-col gap-2">
        {borrowDisabled ? (
          <div className="tooltip w-full" data-tip={tooltip}>
            <Button disabled={true} onClick={() => {}} className="">
              Borrow
            </Button>
          </div>
        ) : (
          <Button onClick={onBorrowClicked} className="w-full">
            Borrow
          </Button>
        )}
      </div>
    </div>
  );
};
