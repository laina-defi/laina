import { Button } from '@components/Button';
import { CompactHealthFactor } from '@components/HealthFactor';
import { Loading } from '@components/Loading';
import { type Loan, useLoans } from '@contexts/loan-context';
import { usePools } from '@contexts/pool-context';
import { formatAPR, formatAmount, toCents, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';

interface LoansViewProps {
  onClose: () => void;
  onRepay: (loan: Loan) => void;
}

const LoansView = ({ onClose, onRepay }: LoansViewProps) => {
  const { loans } = useLoans();
  return (
    <>
      <h3 className="text-xl font-bold tracking-tight mb-8">My Loans</h3>
      {isNil(loans) && <Loading />}
      {loans && loans.length === 0 && <p className="text-base">You have no loans.</p>}
      {loans && loans.length > 0 && (
        <table className="table">
          <thead className="text-base text-grey">
            <tr>
              <th>Loan</th>
              <th>Borrowed</th>
              <th>Collateral</th>
              <th>Health</th>
              <th>Net APR</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {loans.map((loan) => (
              <TableRow key={loan.loanId.nonce} loan={loan} onRepay={onRepay} />
            ))}
          </tbody>
        </table>
      )}
      <div className="modal-action">
        <Button variant="ghost" className="ml-auto" onClick={onClose}>
          Close
        </Button>
      </div>
    </>
  );
};

interface TableRowProps {
  loan: Loan;
  onRepay: (loan: Loan) => void;
}

const TableRow = ({ loan, onRepay }: TableRowProps) => {
  const { borrowedAmount, unpaidInterest, collateralAmount, borrowedTicker, collateralTicker } = loan;
  const { prices, pools } = usePools();

  const loanTotal = borrowedAmount + unpaidInterest;

  const loanPrice = prices?.[borrowedTicker];
  const collateralPrice = prices?.[collateralTicker];

  const borrowed_pool = pools?.[borrowedTicker];
  const collateral_pool = pools?.[collateralTicker];

  const collateralLendedTokens = collateral_pool
    ? collateral_pool.totalBalanceTokens - collateral_pool.availableBalanceTokens
    : 0n;
  const collateralUtilization = collateral_pool?.totalBalanceTokens
    ? Number(collateralLendedTokens) / Number(collateral_pool.totalBalanceTokens)
    : 0;
  const collateralSupplyYield = collateral_pool
    ? BigInt(Math.round(Number(collateral_pool.annualInterestRate) * collateralUtilization))
    : 0n;

  const handleRepayClicked = () => onRepay(loan);

  const loanAmountCents = loanPrice ? toCents(loanPrice, borrowedAmount) : undefined;
  const collateralAmountCents = collateralPrice ? toCents(collateralPrice, collateralAmount) : undefined;

  const net_apr =
    borrowed_pool && collateral_pool && loanAmountCents && collateralAmountCents
      ? BigInt(
          Math.round(
            (Number(borrowed_pool.annualInterestRate) * Number(loanAmountCents) -
              Number(collateralSupplyYield) * Number(collateralAmountCents)) /
              Number(loanAmountCents),
          ),
        )
      : null;

  const healthFactor =
    loanAmountCents && loanAmountCents > 0n ? Number(collateralAmountCents) / Number(loanAmountCents) : 0;

  return (
    <tr key={loan.loanId.nonce} className="text-base">
      <td>{loan.loanId.nonce}</td>
      <td>
        <div>
          <p>
            {formatAmount(loanTotal)} {borrowedTicker}
          </p>
          <p className="text-grey-dark">{loanPrice && toDollarsFormatted(loanPrice, loanTotal)}</p>
        </div>
      </td>
      <td>
        <p>
          {formatAmount(collateralAmount)} {collateralTicker}
        </p>
        <p className="text-grey-dark">{collateralPrice && toDollarsFormatted(collateralPrice, collateralAmount)}</p>
      </td>
      <td>
        <CompactHealthFactor value={healthFactor} />
      </td>
      <td>
        {net_apr ? (
          <div className="relative group">
            <span>{formatAPR(net_apr)}</span>
            <span className="ml-1 text-xs text-gray-500">ⓘ</span>
            <div className="absolute bottom-full left-0 mb-2 px-2 py-1 bg-black text-white text-xs rounded opacity-0 group-hover:opacity-100 transition-opacity duration-200 pointer-events-none whitespace-nowrap z-10">
              <div>Borrowed APR: {borrowed_pool ? formatAPR(borrowed_pool.annualInterestRate) : null}</div>
              <div>Collateral APY: {collateral_pool ? formatAPR(collateralSupplyYield) : null}</div>
            </div>
          </div>
        ) : null}
      </td>
      <td>
        <Button onClick={handleRepayClicked}>Repay</Button>
      </td>
    </tr>
  );
};

export default LoansView;
