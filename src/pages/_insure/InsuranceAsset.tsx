import { Button } from '@components/Button';
import { StellarExpertLink } from '@components/Link';
import { Loading } from '@components/Loading';
import { useInsurancePools } from '@contexts/insurance-pool-context';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { calcInsuranceAPY, calcInsurerLaiAPR, formatAmount, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
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

  return (
    <tr className="border-none text-base h-[6.5rem]">
      <td className="pl-2 pr-6 w-20">
        <img src={icon} alt="" className="mx-auto max-h-12" />
      </td>

      <td>
        <h2 className="font-semibold text-2xl mt-3 tracking-tight">{name}</h2>
        <StellarExpertLink contractId={contractId} text="View insurance pool contract" />
      </td>

      <td>
        <h2 className="text-xl font-semibold mt-3 leading-6">{ticker}</h2>
        <span>{issuerName}</span>
      </td>

      <td>
        <p className="text-xl font-semibold mt-3 leading-6">
          {pool ? formatAmount(pool.totalTokens) : <Loading size="xs" />}
        </p>
        <p>{!isNil(price) && !isNil(pool) && toDollarsFormatted(price, pool.totalTokens)}</p>
      </td>

      <td>
        {lendingPool && pool && price !== undefined ? (() => {
          const baseAPY = calcInsuranceAPY(lendingPool.annualInterestRate, lendingPool.totalBalanceTokens, lendingPool.availableBalanceTokens, pool.totalTokens);
          const laiAPR = calcInsurerLaiAPR(pool.totalTokens, price);
          const netAPY = baseAPY + laiAPR;
          const tip = `Interest earnings: ${baseAPY.toFixed(2)}% from lending income. Plus ${laiAPR.toFixed(2)}% in LAI token rewards per year. Net APY: ${netAPY.toFixed(2)}%.`;
          return (
            <span className="flex items-center gap-1">
              <span className="text-xl font-semibold leading-6">{netAPY.toFixed(2)} %</span>
              <span className="tooltip tooltip-left cursor-help" data-tip={tip}>
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="opacity-40"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>
              </span>
            </span>
          );
        })() : <Loading size="xs" />}
      </td>

      <td>
        {hasInsurancePosition ? (
          <>
            <p className="text-xl font-semibold mt-3 leading-6">{formatAmount(insuranceBalance)}</p>
            <p>{!isNil(price) && toDollarsFormatted(price, insuranceBalance)}</p>
          </>
        ) : (
          <p className="opacity-50">—</p>
        )}
      </td>

      <td className="pr-0">
        <div className="flex flex-col gap-2 items-end">
          {hasLTokens || hasInsurancePosition ? (
            <Button onClick={onManageClicked}>Manage</Button>
          ) : (
            <div className="tooltip" data-tip={!wallet ? 'Connect a wallet first' : 'No lTokens to deposit'}>
              <Button disabled={true} onClick={() => {}}>
                Manage
              </Button>
            </div>
          )}
        </div>
      </td>
    </tr>
  );
};
