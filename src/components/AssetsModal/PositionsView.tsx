import { Button } from '@components/Button';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { formatAmount, formatDepositAPY, toDollarsFormatted } from '@lib/formatting';
import type { SupportedCurrency } from 'currencies';
import { isNil } from 'ramda';
import { CURRENCY_BINDINGS } from 'src/currency-bindings';

export interface PositionsViewProps {
  onClose: () => void;
  onWithdraw: (ticker: SupportedCurrency) => void;
}

const PositionsView = ({ onClose, onWithdraw }: PositionsViewProps) => {
  const { positions } = useWallet();
  const entries = Object.entries(positions).filter(([, { receivable_shares }]) => receivable_shares !== 0n);
  return (
    <>
      <h3 className="text-xl font-bold tracking-tight mb-8">My Assets</h3>
      {/* Mobile card list */}
      <div className="flex flex-col gap-3 md:hidden">
        {entries.map(([ticker, { receivable_shares }]) => (
          <AssetCard
            key={ticker}
            ticker={ticker as SupportedCurrency}
            receivableShares={receivable_shares}
            onWithdraw={onWithdraw}
          />
        ))}
      </div>
      {/* Desktop table */}
      <table className="table hidden md:table">
        <thead className="text-base text-grey">
          <tr>
            <th className="w-20" />
            <th>Asset</th>
            <th>Balance</th>
            <th>APY</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {entries.map(([ticker, { receivable_shares }]) => (
            <TableRow
              key={ticker}
              ticker={ticker as SupportedCurrency}
              receivableShares={receivable_shares}
              onWithdraw={onWithdraw}
            />
          ))}
        </tbody>
      </table>
      <div className="modal-action">
        <Button variant="ghost" className="ml-auto" onClick={onClose}>
          Close
        </Button>
      </div>
    </>
  );
};

interface AssetItemProps {
  receivableShares: bigint;
  ticker: SupportedCurrency;
  onWithdraw: (ticker: SupportedCurrency) => void;
}

const AssetCard = ({ receivableShares, ticker, onWithdraw }: AssetItemProps) => {
  const { prices, pools } = usePools();

  const { icon, name } = CURRENCY_BINDINGS[ticker];
  const price = prices?.[ticker];
  const pool = pools?.[ticker];

  if (!pool) return null;

  const totalBalance = (receivableShares * pool.totalBalanceTokens) / pool.totalBalanceShares;

  return (
    <div className="rounded border border-grey-light p-4">
      <div className="flex items-center gap-3 mb-3">
        <div className="h-10 w-10 shrink-0">
          <img src={icon} alt="" />
        </div>
        <div>
          <p className="font-semibold leading-5">{name}</p>
          <p className="text-sm text-grey">{ticker}</p>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-x-4 gap-y-2 mb-3 text-sm">
        <div>
          <p className="text-grey mb-0.5">Balance</p>
          <p className="font-semibold">
            {formatAmount(totalBalance)} {ticker}
          </p>
          {!isNil(price) && <p className="text-grey-dark text-xs">{toDollarsFormatted(price, totalBalance)}</p>}
        </div>
        <div>
          <p className="text-grey mb-0.5">APY</p>
          <p className="font-semibold">
            {formatDepositAPY(pool.annualInterestRate, pool.totalBalanceTokens, pool.availableBalanceTokens)}
          </p>
        </div>
      </div>
      <Button onClick={() => onWithdraw(ticker)}>Withdraw</Button>
    </div>
  );
};

interface TableRowProps {
  receivableShares: bigint;
  ticker: SupportedCurrency;
  onWithdraw: (ticker: SupportedCurrency) => void;
}

const TableRow = ({ receivableShares, ticker, onWithdraw }: TableRowProps) => {
  const { prices, pools } = usePools();

  const { icon, name } = CURRENCY_BINDINGS[ticker];
  const price = prices?.[ticker];
  const pool = pools?.[ticker];

  if (!pool) {
    console.warn('PoolState is not loaded');
    return null;
  }

  const totalBalance = (receivableShares * pool.totalBalanceTokens) / pool.totalBalanceShares;

  return (
    <tr key={ticker}>
      <td>
        <div className="h-12 w-12">
          <img src={icon} alt="" />
        </div>
      </td>
      <td>
        <div>
          <p className="text-lg font-semibold leading-5">{name}</p>
          <p className="text-base">{ticker}</p>
        </div>
      </td>
      <td>
        <p className="text-lg font-semibold leading-5">{formatAmount(totalBalance)}</p>
        <p className="text-base">{!isNil(price) && toDollarsFormatted(price, totalBalance)}</p>
      </td>
      <td className="text-lg font-semibold">
        {pool && formatDepositAPY(pool.annualInterestRate, pool.totalBalanceTokens, pool.availableBalanceTokens)}
      </td>
      <td>
        <Button onClick={() => onWithdraw(ticker)}>Withdraw</Button>
      </td>
    </tr>
  );
};

export default PositionsView;
