import { Button } from '@components/Button';
import { Loading } from '@components/Loading';
import { usePools } from '@contexts/pool-context';
import { type Balance, useWallet } from '@contexts/wallet-context';
import { isBalanceZero } from '@lib/converters';
import { formatAPY, formatAmount, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
import type { CurrencyBinding } from 'src/currency-bindings';

export interface LendMobileCardProps {
  currency: CurrencyBinding;
  onDepositClicked: VoidFunction;
}

export const LendMobileCard = ({ currency, onDepositClicked }: LendMobileCardProps) => {
  const { icon, name, ticker, issuerName } = currency;

  const { wallet, walletBalances } = useWallet();
  const { prices, pools } = usePools();
  const pool = pools?.[ticker];
  const price = prices?.[ticker];
  const balance: Balance | undefined = walletBalances?.[ticker];

  const isPoor = !balance?.trustLine || isBalanceZero(balance.balanceLine.balance);

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
          <p className="text-sm opacity-70 mb-1">Balance</p>
          <p className="font-semibold text-lg">
            {pool ? formatAmount(pool.totalBalanceTokens) : <Loading size="xs" />}
          </p>
          <p className="text-sm opacity-70">
            {!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.totalBalanceTokens)}
          </p>
        </div>
        <div>
          <p className="text-sm opacity-70 mb-1">Supply APY</p>
          <p className="font-semibold text-lg">{pool ? formatAPY(pool.annualInterestRate) : <Loading size="xs" />}</p>
        </div>
      </div>

      <div className="flex flex-col gap-2">
        {isPoor ? (
          <div className="tooltip w-full" data-tip={!wallet ? 'Connect a wallet first' : 'Not enough funds'}>
            <Button disabled={true} onClick={() => {}} className="">
              Deposit
            </Button>
          </div>
        ) : (
          <Button onClick={onDepositClicked} className="w-full">
            Deposit
          </Button>
        )}
      </div>
    </div>
  );
};
