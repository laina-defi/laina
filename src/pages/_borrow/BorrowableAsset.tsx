import { useMemo } from 'react';

import { Button } from '@components/Button';
import { StellarExpertLink } from '@components/Link';
import { Loading } from '@components/Loading';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { isBalanceZero } from '@lib/converters';
import { calcBorrowerLaiAPR, formatAmount, toDollarsFormatted } from '@lib/formatting';
import type { CurrencyBinding } from 'src/currency-bindings';

interface BorrowableAssetCardProps {
  currency: CurrencyBinding;
  onBorrowClicked: VoidFunction;
}

export const BorrowableAsset = ({ currency, onBorrowClicked }: BorrowableAssetCardProps) => {
  const { icon, name, ticker, issuerName, contractId } = currency;

  const { wallet, walletBalances } = useWallet();
  const { prices, pools } = usePools();
  const price = prices?.[ticker];
  const pool = pools?.[ticker];

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
    <tr className="border-none text-base h-[6.5rem]">
      <td className="w-20 pl-2 pr-6">
        <img src={icon} alt="" className="mx-auto max-h-12" />
      </td>

      <td>
        <h2 className="font-semibold text-2xl mt-3 tracking-tight">{name}</h2>
        <StellarExpertLink contractId={contractId} text="View pool contract" />
      </td>

      <td>
        <h2 className="text-xl font-semibold mt-3 leading-6">{ticker}</h2>
        <span>{issuerName}</span>
      </td>

      <td>
        <p className="text-xl font-semibold mt-3 leading-6">
          {pool ? formatAmount(pool.availableBalanceTokens) : <Loading size="xs" />}
        </p>
        <p>{pool && price ? toDollarsFormatted(price, pool.availableBalanceTokens) : null}</p>
      </td>

      <td>
        {pool && price !== undefined ? (() => {
          const baseAPR = Number(pool.annualInterestRate) / 100_000;
          const laiAPR = calcBorrowerLaiAPR(pool.totalBalanceTokens, pool.availableBalanceTokens, price);
          const netAPR = baseAPR - laiAPR;
          const tip = `Base borrow rate: ${baseAPR.toFixed(2)}%. You earn LAI token rewards worth ${laiAPR.toFixed(2)}% of your borrowed amount per year, reducing your net cost to ${netAPR.toFixed(2)}%.`;
          return (
            <span className="flex items-center gap-1">
              <span className="text-xl font-semibold leading-6">{netAPR.toFixed(2)} %</span>
              <span className="tooltip tooltip-left cursor-help" data-tip={tip}>
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="opacity-40"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>
              </span>
            </span>
          );
        })() : <Loading size="xs" />}
      </td>

      <td className="pr-0">
        {borrowDisabled ? (
          <div className="tooltip" data-tip={tooltip}>
            <Button disabled={true} onClick={() => {}}>
              Borrow
            </Button>
          </div>
        ) : (
          <Button onClick={onBorrowClicked}>Borrow</Button>
        )}
      </td>
    </tr>
  );
};
