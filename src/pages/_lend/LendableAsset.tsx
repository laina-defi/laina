import { Button } from '@components/Button';
import { StellarExpertLink } from '@components/Link';
import { Loading } from '@components/Loading';
import { type PoolStatus, usePools } from '@contexts/pool-context';
import { type Balance, useWallet } from '@contexts/wallet-context';
import { isBalanceZero } from '@lib/converters';
import { formatAmount, formatDepositAPY, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
import { FaCircleCheck, FaCircleExclamation, FaCircleXmark, FaLock } from 'react-icons/fa6';
import type { CurrencyBinding } from 'src/currency-bindings';

const statusConfig: Record<PoolStatus, { color: string; icon: React.ReactNode; tip: string }> = {
  Healthy: {
    color: 'badge-success',
    icon: <FaCircleCheck />,
    tip: 'Borrowing and deposits are open. Insurance pool covers ≥10% of pool balance.',
  },
  Caution: {
    color: 'badge-warning',
    icon: <FaCircleExclamation />,
    tip: 'Deposits allowed, but borrowing is disabled. Insurance pool is below the 10% coverage threshold.',
  },
  Restricted: {
    color: 'badge-error',
    icon: <FaCircleXmark />,
    tip: 'New deposits and borrowing are disabled. Repayments and liquidations still work.',
  },
  Frozen: {
    color: 'badge-neutral',
    icon: <FaLock />,
    tip: 'Pool is frozen. Only repayments and liquidations are permitted.',
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

  return (
    <tr className="border-none text-base h-[6.5rem]">
      <td className="pl-2 pr-6 w-20">
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
          {pool ? formatAmount(pool.totalBalanceTokens) : <Loading size="xs" />}
        </p>
        <p>{!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.totalBalanceTokens)}</p>
      </td>

      <td>
        <p className="text-xl font-semibold leading-6">
          {pool ? formatDepositAPY(pool.annualInterestRate, pool.totalBalanceTokens, pool.availableBalanceTokens) : <Loading size="xs" />}
        </p>
      </td>

      <td>
        {pool ? (() => {
          const { color, icon, tip } = statusConfig[pool.poolStatus];
          return (
            <div className="tooltip tooltip-left" data-tip={tip}>
              <span className={`badge badge-outline gap-1.5 ${color}`}>
                {icon}
                {pool.poolStatus}
              </span>
            </div>
          );
        })() : (
          <Loading size="xs" />
        )}
      </td>

      <td className="pr-0">
        {isPoor ? (
          <div className="tooltip" data-tip={!wallet ? 'Connect a wallet first' : 'Not enough funds'}>
            <Button disabled={true} onClick={() => {}}>
              Deposit
            </Button>
          </div>
        ) : (
          <Button onClick={onDepositClicked}>Deposit</Button>
        )}
      </td>
    </tr>
  );
};
