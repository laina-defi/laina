import { Button } from '@components/Button';
import { StellarExpertLink } from '@components/Link';
import { Loading } from '@components/Loading';
import { type PoolStatus, usePools } from '@contexts/pool-context';
import { type Balance, useWallet } from '@contexts/wallet-context';
import { isBalanceZero } from '@lib/converters';
import { formatAmount, formatDepositAPY, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
import { PiCheckCircleFill, PiInfo, PiLockSimpleFill, PiWarningCircleFill, PiXCircleFill } from 'react-icons/pi';
import type { CurrencyBinding } from 'src/currency-bindings';

const statusConfig: Record<PoolStatus, { iconClass: string; icon: React.ReactNode; tip: string }> = {
  Healthy: {
    iconClass: 'text-success',
    icon: <PiCheckCircleFill size={22} />,
    tip: 'Healthy — borrowing and deposits are open.',
  },
  Caution: {
    iconClass: 'text-warning',
    icon: <PiWarningCircleFill size={22} />,
    tip: 'Caution — deposits allowed, borrowing disabled. Insurance pool below 10% threshold.',
  },
  Restricted: {
    iconClass: 'text-error',
    icon: <PiXCircleFill size={22} />,
    tip: 'Restricted — new deposits and borrowing are disabled.',
  },
  Frozen: {
    iconClass: 'text-grey',
    icon: <PiLockSimpleFill size={22} />,
    tip: 'Frozen — only repayments and liquidations are permitted.',
  },
};

export interface LendableAssetProps {
  currency: CurrencyBinding;
  onDepositClicked: VoidFunction;
}

export const LendableAsset = ({ currency, onDepositClicked }: LendableAssetProps) => {
  const { icon, name, ticker, issuerName, contractId } = currency;

  const { wallet, walletBalances } = useWallet();
  const { prices, pools } = usePools();
  const pool = pools?.[ticker];
  const price = prices?.[ticker];
  const balance: Balance | undefined = walletBalances?.[ticker];

  const isPoor = !balance?.trustLine || isBalanceZero(balance.balanceLine.balance);

  const utilization =
    pool && pool.totalBalanceTokens > 0n
      ? Number(((pool.totalBalanceTokens - pool.availableBalanceTokens) * 100n) / pool.totalBalanceTokens)
      : 0;

  const status = pool ? statusConfig[pool.poolStatus] : null;

  const actionButton = isPoor ? (
    <div className="tooltip w-full md:w-auto" data-tip={!wallet ? 'Connect a wallet first' : 'Not enough funds'}>
      <Button disabled={true} onClick={() => {}} className="w-full md:w-auto">
        Deposit
      </Button>
    </div>
  ) : (
    <Button onClick={onDepositClicked} className="w-full md:w-auto">
      Deposit
    </Button>
  );

  return (
    <div className="border-b border-grey-light last:border-none hover:bg-grey-lighter/30 transition-colors">
      {/* ── Mobile card layout (hidden at md+) ── */}
      <div className="md:hidden p-4">
        <div className="flex items-center gap-3 mb-4">
          <img src={icon} alt="" className="w-10 h-10 flex-none" />
          <div className="flex-1 min-w-0">
            <h2 className="font-semibold text-base tracking-tight truncate">{name}</h2>
            <p className="text-xs text-grey">
              {ticker} · {issuerName}
            </p>
          </div>
          {status && (
            <span className={`tooltip tooltip-left cursor-help flex-none ${status.iconClass}`} data-tip={status.tip}>
              {status.icon}
            </span>
          )}
        </div>

        <div className="grid grid-cols-3 gap-3 mb-4">
          <div>
            <p className="text-xs text-grey mb-0.5">Deposits</p>
            <p className="text-sm font-semibold leading-5">
              {pool ? formatAmount(pool.totalBalanceTokens) : <Loading size="xs" />}
            </p>
            <p className="text-xs text-grey">
              {!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.totalBalanceTokens)}
            </p>
          </div>
          <div>
            <p className="text-xs text-grey mb-0.5">Supply APY</p>
            <p className="text-sm font-semibold leading-5">
              {pool ? (
                formatDepositAPY(pool.annualInterestRate, pool.totalBalanceTokens, pool.availableBalanceTokens)
              ) : (
                <Loading size="xs" />
              )}
            </p>
          </div>
          <div>
            <p className="text-xs text-grey mb-0.5">Utilization</p>
            <p className="text-sm font-semibold leading-5">{pool ? `${utilization} %` : <Loading size="xs" />}</p>
          </div>
        </div>

        <StellarExpertLink contractId={contractId} text="View pool contract" className="text-sm mb-3" />

        {actionButton}
      </div>

      {/* ── Desktop grid row layout (hidden below md) ── */}
      <div className="hidden md:grid md:grid-cols-[80px_1fr_40px_90px_150px_120px_130px_130px] md:items-center md:min-h-[6.5rem] md:px-1">
        {/* Icon */}
        <div className="flex justify-center">
          <img src={icon} alt="" className="max-h-12 max-w-[48px]" />
        </div>

        {/* Name + link */}
        <div>
          <h2 className="font-semibold text-xl tracking-tight">{name}</h2>
          <StellarExpertLink contractId={contractId} text="View pool contract" />
        </div>

        {/* Status icon */}
        <div className="flex items-center">
          {status && (
            <span className={`tooltip tooltip-right cursor-help ${status.iconClass}`} data-tip={status.tip}>
              {status.icon}
            </span>
          )}
        </div>

        {/* Ticker */}
        <div>
          <p className="font-semibold leading-5">{ticker}</p>
          <p className="text-sm text-grey">{issuerName}</p>
        </div>

        {/* Deposits */}
        <div>
          <p className="text-lg font-semibold leading-5">
            {pool ? formatAmount(pool.totalBalanceTokens) : <Loading size="xs" />}
          </p>
          <p className="text-sm text-grey">
            {!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.totalBalanceTokens)}
          </p>
        </div>

        {/* Supply APY */}
        <div>
          <p className="text-lg font-semibold leading-5">
            {pool ? (
              formatDepositAPY(pool.annualInterestRate, pool.totalBalanceTokens, pool.availableBalanceTokens)
            ) : (
              <Loading size="xs" />
            )}
          </p>
        </div>

        {/* Utilization */}
        <div>
          <p className="text-lg font-semibold leading-5">{pool ? `${utilization} %` : <Loading size="xs" />}</p>
        </div>

        {/* Action */}
        <div className="flex justify-end pr-2 py-2">
          {actionButton}
        </div>
      </div>
    </div>
  );
};

export const InfoIcon = () => (
  <span className="tooltip cursor-help opacity-40 hover:opacity-70 transition-opacity">
    <PiInfo size={14} />
  </span>
);
