import { Button } from '@components/Button';
import { StellarExpertLink } from '@components/Link';
import { Loading } from '@components/Loading';
import { useInsurancePools } from '@contexts/insurance-pool-context';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { calcInsuranceAPY, calcInsurerLaiAPR, formatAmount, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
import { PiInfo } from 'react-icons/pi';
import type { CurrencyBinding } from 'src/insurance-bindings';

export interface InsuranceAssetProps {
  currency: CurrencyBinding;
  onManageClicked: VoidFunction;
}

export const InsuranceAsset = ({ currency, onManageClicked }: InsuranceAssetProps) => {
  const { icon, name, ticker, issuerName, contractId } = currency;

  const { wallet, positions, insurancePositions } = useWallet();
  const { prices, pools } = useInsurancePools();
  const { pools: lendingPools } = usePools();
  const pool = pools?.[ticker];
  const lendingPool = lendingPools?.[ticker];
  const price = prices?.[ticker];

  const lTokenBalance = positions?.[ticker]?.receivable_shares ?? 0n;
  const insuranceBalance = insurancePositions?.[ticker] ?? 0n;

  const hasLTokens = wallet && lTokenBalance > 0n;
  const hasInsurancePosition = wallet && insuranceBalance > 0n;

  const coverage =
    lendingPool && pool && lendingPool.totalBalanceTokens > 0n
      ? Number((pool.totalTokens * 100n) / lendingPool.totalBalanceTokens)
      : null;

  const apyContent =
    lendingPool && pool && price !== undefined ? (
      (() => {
        const baseAPY = calcInsuranceAPY(
          lendingPool.annualInterestRate,
          lendingPool.totalBalanceTokens,
          lendingPool.availableBalanceTokens,
          pool.totalTokens,
        );
        const laiAPR = calcInsurerLaiAPR(pool.totalTokens, price);
        const netAPY = baseAPY + laiAPR;
        const tip = `Interest earnings: ${baseAPY.toFixed(2)}% from lending income. Plus ${laiAPR.toFixed(2)}% in LAI token rewards per year. Net APY: ${netAPY.toFixed(2)}%.`;
        return (
          <span className="flex items-center gap-1">
            <span className="font-semibold leading-5">{netAPY.toFixed(2)} %</span>
            <span className="tooltip tooltip-left cursor-help" data-tip={tip}>
              <PiInfo size={14} className="opacity-40 hover:opacity-70 transition-opacity" />
            </span>
          </span>
        );
      })()
    ) : (
      <Loading size="xs" />
    );

  const actionButton =
    hasLTokens || hasInsurancePosition ? (
      <Button onClick={onManageClicked} className="w-full md:w-auto">
        Manage
      </Button>
    ) : (
      <div className="tooltip w-full md:w-auto" data-tip={!wallet ? 'Connect a wallet first' : 'No lTokens to deposit'}>
        <Button disabled={true} onClick={() => {}} className="w-full md:w-auto">
          Manage
        </Button>
      </div>
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
            <p className="text-xs text-grey mb-0.5">Pool TVL</p>
            <p className="text-sm font-semibold leading-5">
              {pool ? formatAmount(pool.totalTokens) : <Loading size="xs" />}
            </p>
            <p className="text-xs text-grey">
              {!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.totalTokens)}
            </p>
          </div>
          <div>
            <p className="text-xs text-grey mb-0.5">APY</p>
            <div className="text-sm">{apyContent}</div>
          </div>
          <div>
            <p className="text-xs text-grey mb-0.5">Coverage</p>
            <p className="text-sm font-semibold leading-5">
              {coverage !== null ? `${coverage} %` : <Loading size="xs" />}
            </p>
          </div>
          {hasInsurancePosition && (
            <div>
              <p className="text-xs text-grey mb-0.5">My Position</p>
              <p className="text-sm font-semibold leading-5">{formatAmount(insuranceBalance)}</p>
              <p className="text-xs text-grey">{!isNil(price) && toDollarsFormatted(price, insuranceBalance)}</p>
            </div>
          )}
        </div>

        <StellarExpertLink contractId={contractId} text="View insurance pool contract" className="text-sm mb-3" />

        {actionButton}
      </div>

      {/* ── Desktop grid row layout ── */}
      <div className="hidden md:grid md:grid-cols-[80px_1fr_90px_150px_110px_100px_150px_130px] md:items-center md:min-h-[6.5rem] md:px-1">
        {/* Icon */}
        <div className="flex justify-center">
          <img src={icon} alt="" className="max-h-12 max-w-[48px]" />
        </div>

        {/* Name + link */}
        <div>
          <h2 className="font-semibold text-xl tracking-tight">{name}</h2>
          <StellarExpertLink contractId={contractId} text="View insurance pool" />
        </div>

        {/* Ticker */}
        <div>
          <p className="font-semibold leading-5">{ticker}</p>
          <p className="text-sm text-grey">{issuerName}</p>
        </div>

        {/* Pool TVL */}
        <div>
          <p className="text-lg font-semibold leading-5">
            {pool ? formatAmount(pool.totalTokens) : <Loading size="xs" />}
          </p>
          <p className="text-sm text-grey">
            {!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.totalTokens)}
          </p>
        </div>

        {/* APY */}
        <div className="text-lg">{apyContent}</div>

        {/* Coverage */}
        <div>
          <p className="text-lg font-semibold leading-5">
            {coverage !== null ? `${coverage} %` : <Loading size="xs" />}
          </p>
        </div>

        {/* My Position */}
        <div>
          {hasInsurancePosition ? (
            <>
              <p className="text-lg font-semibold leading-5">{formatAmount(insuranceBalance)}</p>
              <p className="text-sm text-grey">{!isNil(price) && toDollarsFormatted(price, insuranceBalance)}</p>
            </>
          ) : (
            <p className="text-grey opacity-50">—</p>
          )}
        </div>

        {/* Action */}
        <div className="flex justify-end pr-2 py-2">{actionButton}</div>
      </div>
    </div>
  );
};
