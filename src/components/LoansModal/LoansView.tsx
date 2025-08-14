import { Button } from '@components/Button';
import { CompactHealthFactor } from '@components/HealthFactor';
import { Loading } from '@components/Loading';
import { type Loan, useLoans } from '@contexts/loan-context';
import { usePools } from '@contexts/pool-context';
import { formatAPR, formatAmount, toCents, toDollarsFormatted } from '@lib/formatting';
import { isNil } from 'ramda';
import { CURRENCY_BINDINGS } from 'src/currency-bindings';

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
              <th>APR</th>
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

  const pool = pools?.[borrowedTicker];

  const handleRepayClicked = () => onRepay(loan);

  const loanAmountCents = loanPrice ? toCents(loanPrice, borrowedAmount) : undefined;
  const collateralAmountCents = collateralPrice ? toCents(collateralPrice, collateralAmount) : undefined;

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
      <td>{pool ? formatAPR(pool.annualInterestRate) : null}</td>
      <td>
        <Button onClick={handleRepayClicked}>Repay</Button>
      </td>
    </tr>
  );
};

export default LoansView;
