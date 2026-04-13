import { Button } from '@components/Button';
import { Loading } from '@components/Loading';
import { useInsurancePools } from '@contexts/insurance-pool-context';
import { useWallet } from '@contexts/wallet-context';
import { formatAmount, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
import type { CurrencyBinding } from 'src/insurance-bindings';

export interface InsureMobileCardProps {
  currency: CurrencyBinding;
  onManageClicked: VoidFunction;
}

export const InsureMobileCard = ({ currency, onManageClicked }: InsureMobileCardProps) => {
  const { icon, name, ticker, issuerName } = currency;

  const { wallet, positions, insurancePositions } = useWallet();
  const { prices, pools } = useInsurancePools();
  const pool = pools?.[ticker];
  const price = prices?.[ticker];

  const lTokenBalance = positions?.[ticker]?.receivable_shares ?? 0n;
  const insuranceBalance = insurancePositions?.[ticker] ?? 0n;

  const hasLTokens = wallet && lTokenBalance > 0n;
  const hasInsurancePosition = wallet && insuranceBalance > 0n;

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
          <p className="text-sm opacity-70 mb-1">Pool TVL</p>
          <p className="font-semibold text-lg">
            {pool ? formatAmount(pool.totalTokens) : <Loading size="xs" />}
          </p>
          <p className="text-sm opacity-70">
            {!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.totalTokens)}
          </p>
        </div>
        <div>
          <p className="text-sm opacity-70 mb-1">APY</p>
          <p className="font-semibold text-lg">—</p>
        </div>
        {hasInsurancePosition && (
          <div>
            <p className="text-sm opacity-70 mb-1">Your Position</p>
            <p className="font-semibold text-lg">{formatAmount(insuranceBalance)}</p>
            <p className="text-sm opacity-70">
              {!isNil(price) && toDollarsFormatted(price, insuranceBalance)}
            </p>
          </div>
        )}
      </div>

      <div className="flex flex-col gap-2">
        {hasLTokens || hasInsurancePosition ? (
          <Button onClick={onManageClicked} className="w-full">
            Manage
          </Button>
        ) : (
          <div className="tooltip w-full" data-tip={!wallet ? 'Connect a wallet first' : 'No lTokens to deposit'}>
            <Button disabled={true} onClick={() => {}} className="w-full">
              Manage
            </Button>
          </div>
        )}
      </div>
    </div>
  );
};
