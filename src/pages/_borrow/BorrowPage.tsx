import { useState } from 'react';

import { Card } from '@components/Card';
import { StellarExpertLink } from '@components/Link';
import { Table } from '@components/Table';
import WalletCard from '@components/WalletCard/WalletCard';
import { contractId } from '@contracts/loan_manager';
import { CURRENCY_BINDINGS_ARR, type CurrencyBinding } from 'src/currency-bindings';
import { BorrowModal } from './BorrowModal/BorrowModal';
import { BorrowableAsset } from './BorrowableAsset';

const links = [
  { to: '/lend', label: 'Lend' },
  { to: '/borrow', label: 'Borrow' },
];

const BorrowPage = () => {
  const [selectedCurrency, setSelectedCurrency] = useState<CurrencyBinding | null>(null);

  const modalId = 'borrow-modal';

  const openBorrowModal = (currency: CurrencyBinding) => {
    setSelectedCurrency(currency);
    const modalEl = document.getElementById(modalId) as HTMLDialogElement;
    modalEl.showModal();
  };

  const closeBorrowModal = () => {
    const modalEl = document.getElementById(modalId) as HTMLDialogElement;
    modalEl.close();
    setSelectedCurrency(null);
  };

  return (
    <>
      <div className="my-14">
        <WalletCard />
        <Card links={links}>
          <div className="px-12 pb-12 pt-4">
            <h1 className="text-2xl font-semibold mb-4 tracking-tight">Borrow Assets</h1>
            <Table headers={['Asset', null, 'Ticker', 'Balance', 'Borrow APY', null]}>
              {CURRENCY_BINDINGS_ARR.map((currency) => (
                <BorrowableAsset
                  key={currency.ticker}
                  currency={currency}
                  onBorrowClicked={() => openBorrowModal(currency)}
                />
              ))}
            </Table>
            <StellarExpertLink className="mt-3" contractId={contractId} text="View Loan Manager contract" />
          </div>
        </Card>
      </div>
      <BorrowModal modalId={modalId} onClose={closeBorrowModal} currency={selectedCurrency} />
    </>
  );
};

export default BorrowPage;
