import { Dialog } from '@components/Dialog';
import type { Loan } from '@contexts/loan-context';
import { isNil } from 'ramda';
import { useState } from 'react';
import AdjustCollateralView from './AdjustCollateralView';
import LoansView from './LoansView';
import RepayView from './RepayView';

export interface LoansModalProps {
  modalId: string;
  onClose: () => void;
}

type ActiveView = 'list' | 'repay' | 'adjust-collateral';

const LoansModal = ({ modalId, onClose }: LoansModalProps) => {
  const [selectedLoan, setSelectedLoan] = useState<Loan | null>(null);
  const [activeView, setActiveView] = useState<ActiveView>('list');

  const handleBackClicked = () => {
    setSelectedLoan(null);
    setActiveView('list');
  };

  const handleClose = () => {
    setSelectedLoan(null);
    setActiveView('list');
    onClose();
  };

  const handleRepayClicked = (loan: Loan) => {
    setSelectedLoan(loan);
    setActiveView('repay');
  };

  const handleAdjustCollateralClicked = (loan: Loan) => {
    setSelectedLoan(loan);
    setActiveView('adjust-collateral');
  };

  return (
    <Dialog modalId={modalId} onClose={handleClose} className="min-w-96">
      {activeView === 'list' && (
        <LoansView
          onClose={handleClose}
          onRepay={handleRepayClicked}
          onAdjustCollateral={handleAdjustCollateralClicked}
        />
      )}
      {activeView === 'repay' && !isNil(selectedLoan) && <RepayView loan={selectedLoan} onBack={handleBackClicked} />}
      {activeView === 'adjust-collateral' && !isNil(selectedLoan) && (
        <AdjustCollateralView loan={selectedLoan} onBack={handleBackClicked} />
      )}
    </Dialog>
  );
};

export default LoansModal;
