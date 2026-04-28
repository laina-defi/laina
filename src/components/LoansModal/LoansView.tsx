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
  onAdjustCollateral: (loan: Loan) => void;
}

const LoansView = ({ onClose, onRepay, onAdjustCollateral }: LoansViewProps) => {
  const { loans } = useLoans();
  return (
    <>
      <h3 className="text-xl font-bold tracking-tight mb-6">My Loans</h3>
      {isNil(loans) && <Loading />}
      {loans && loans.length === 0 && <p className="text-base">You have no loans.</p>}
      {loans && loans.length > 0 && (
        <>
          {/* Mobile card list */}
          <div className="flex flex-col gap-3 md:hidden">
            {loans.map((loan) => (
              <LoanCard key={loan.loanId.nonce} loan={loan} onRepay={onRepay} onAdjustCollateral={onAdjustCollateral} />
            ))}
          </div>
          {/* Desktop table */}
          <table className="table hidden md:table">
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
                <TableRow key={loan.loanId.nonce} loan={loan} onRepay={onRepay} onAdjustCollateral={onAdjustCollateral} />
              ))}
            </tbody>
          </table>
        </>
      )}
      <div className="modal-action">
        <Button variant="ghost" className="ml-auto" onClick={onClose}>
          Close
        </Button>
      </div>
    </>
  );
};

interface LoanItemProps {
  loan: Loan;
  onRepay: (loan: Loan) => void;
  onAdjustCollateral: (loan: Loan) => void;
}

const LoanCard = ({ loan, onRepay, onAdjustCollateral }: LoanItemProps) => {
  const { borrowedAmount, unpaidInterest, collateralAmount, borrowedTicker, collateralTicker } = loan;
  const { prices, pools } = usePools();

  const loanTotal = borrowedAmount + unpaidInterest;
  const loanPrice = prices?.[borrowedTicker];
  const collateralPrice = prices?.[collateralTicker];
  const pool = pools?.[borrowedTicker];

  const loanAmountCents = loanPrice ? toCents(loanPrice, borrowedAmount) : undefined;
  const collateralAmountCents = collateralPrice ? toCents(collateralPrice, collateralAmount) : undefined;
  const healthFactor =
    loanAmountCents && loanAmountCents > 0n
      ? (Number(collateralAmountCents) * 0.8) / Number(loanAmountCents)
      : 0;

  return (
    <div className="rounded border border-grey-light p-4">
      <div className="flex items-center justify-between mb-3">
        <span className="text-sm text-grey">Loan #{loan.loanId.nonce}</span>
        <CompactHealthFactor value={healthFactor} />
      </div>
      <div className="grid grid-cols-2 gap-x-4 gap-y-2 mb-3 text-sm">
        <div>
          <p className="text-grey mb-0.5">Borrowed</p>
          <p className="font-semibold">{formatAmount(loanTotal)} {borrowedTicker}</p>
          {loanPrice && <p className="text-grey-dark text-xs">{toDollarsFormatted(loanPrice, loanTotal)}</p>}
        </div>
        <div>
          <p className="text-grey mb-0.5">Collateral</p>
          <p className="font-semibold">{formatAmount(collateralAmount)} {collateralTicker}</p>
          {collateralPrice && <p className="text-grey-dark text-xs">{toDollarsFormatted(collateralPrice, collateralAmount)}</p>}
        </div>
        {pool && (
          <div>
            <p className="text-grey mb-0.5">APR</p>
            <p className="font-semibold">{formatAPR(pool.annualInterestRate)}</p>
          </div>
        )}
      </div>
      <div className="flex gap-2">
        <Button onClick={() => onRepay(loan)}>Repay</Button>
        <Button variant="ghost" onClick={() => onAdjustCollateral(loan)}>Collateral</Button>
      </div>
    </div>
  );
};

interface TableRowProps {
  loan: Loan;
  onRepay: (loan: Loan) => void;
  onAdjustCollateral: (loan: Loan) => void;
}

const TableRow = ({ loan, onRepay, onAdjustCollateral }: TableRowProps) => {
  const { borrowedAmount, unpaidInterest, collateralAmount, borrowedTicker, collateralTicker } = loan;
  const { prices, pools } = usePools();

  const loanTotal = borrowedAmount + unpaidInterest;

  const loanPrice = prices?.[borrowedTicker];
  const collateralPrice = prices?.[collateralTicker];

  const pool = pools?.[borrowedTicker];

  const loanAmountCents = loanPrice ? toCents(loanPrice, borrowedAmount) : undefined;
  const collateralAmountCents = collateralPrice ? toCents(collateralPrice, collateralAmount) : undefined;

  const healthFactor =
    loanAmountCents && loanAmountCents > 0n
      ? (Number(collateralAmountCents) * 0.8) / Number(loanAmountCents)
      : 0;

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
      <td className="flex flex-col gap-2">
        <Button onClick={() => onRepay(loan)}>Repay</Button>
        <Button variant="ghost" onClick={() => onAdjustCollateral(loan)}>
          Collateral
        </Button>
      </td>
    </tr>
  );
};

export default LoansView;
