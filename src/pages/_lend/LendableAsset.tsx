import { Button } from '@components/Button';
import { StellarExpertLink } from '@components/Link';
import { Loading } from '@components/Loading';
import { usePools } from '@contexts/pool-context';
import { type Balance, useWallet } from '@contexts/wallet-context';
import { isBalanceZero } from '@lib/converters';
import { formatAPY, formatAmount, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
import type { CurrencyBinding } from 'src/currency-bindings';

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

  const lendedTokens = pool ? pool.totalBalanceTokens - pool.availableBalanceTokens : 0n;
  const utilization = pool?.totalBalanceTokens ? Number(lendedTokens) / Number(pool.totalBalanceTokens) : 0;
  const supplyYield = pool ? BigInt(Math.round(Number(pool.annualInterestRate) * utilization)) : 0n;

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
        <p className="text-xl font-semibold leading-6">{pool ? formatAPY(supplyYield) : <Loading size="xs" />}</p>
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
