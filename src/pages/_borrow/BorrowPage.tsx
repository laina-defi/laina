import { useState } from 'react';

import { Card } from '@components/Card';
import { StellarExpertLink } from '@components/Link';
import WalletCard from '@components/WalletCard/WalletCard';
import { usePools } from '@contexts/pool-context';
import { useWallet } from '@contexts/wallet-context';
import { contractId } from '@contracts/loan_manager';
import { CURRENCY_BINDINGS_ARR, type CurrencyBinding } from 'src/currency-bindings';
import { BorrowModal } from './BorrowModal/BorrowModal';
import { BorrowableAsset } from './BorrowableAsset';

const links = [
  { to: '/lend', label: 'Lend' },
  { to: '/borrow', label: 'Borrow' },
  { to: '/insure', label: 'Insure' },
  { to: '/auctions', label: 'Auctions' },
];

const BorrowPage = () => {
  const { refetchBalances } = useWallet();
  const { refetchPools } = usePools();
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
    refetchBalances();
    refetchPools();
  };

  return (
    <>
      <div className="my-14">
        <WalletCard />
        <Card links={links}>
          <div className="px-6 md:px-12 pb-12 pt-4">
            <div className="mb-2">
              <h1 className="text-2xl font-semibold tracking-tight">Borrow Assets</h1>
              <p className="text-grey mt-1">Borrow against your collateral. LAI rewards offset your borrowing cost.</p>
            </div>

            {/* Desktop column headers */}
            <div className="hidden md:grid md:grid-cols-[80px_1fr_90px_150px_150px_130px_40px] px-1 pb-2 mt-6 border-b border-grey-light">
              <div />
              <div className="text-sm font-semibold text-grey">Asset</div>
              <div className="text-sm font-semibold text-grey">Ticker</div>
              <div className="text-sm font-semibold text-grey">Available</div>
              <div className="text-sm font-semibold text-grey">Borrow APR</div>
              <div />
              <div />
            </div>

            <div>
              {CURRENCY_BINDINGS_ARR.map((currency) => (
                <BorrowableAsset
                  key={currency.ticker}
                  currency={currency}
                  onBorrowClicked={() => openBorrowModal(currency)}
                />
              ))}
            </div>

            <StellarExpertLink className="mt-4 text-sm" contractId={contractId} text="View Loan Manager contract" />
          </div>
        </Card>
      </div>
      <BorrowModal modalId={modalId} onClose={closeBorrowModal} currency={selectedCurrency} />
    </>
  );
};

export default BorrowPage;
