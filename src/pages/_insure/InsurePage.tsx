import { useState } from 'react';

import { Card } from '@components/Card';
import { StellarExpertLink } from '@components/Link';
import WalletCard from '@components/WalletCard/WalletCard';
import { useInsurancePools } from '@contexts/insurance-pool-context';
import { contractId } from '@contracts/loan_manager';
import { CURRENCY_BINDINGS_ARR, type CurrencyBinding } from 'src/insurance-bindings';
import { InsuranceAsset } from './InsuranceAsset';
import { InsureManageModal } from './InsureManageModal';

const links = [
  { to: '/lend', label: 'Lend' },
  { to: '/borrow', label: 'Borrow' },
  { to: '/insure', label: 'Insure' },
  { to: '/auctions', label: 'Auctions' },
];

const InsurePage = () => {
  const { refetchPools } = useInsurancePools();
  const [selectedCurrency, setSelectedCurrency] = useState<CurrencyBinding | null>(null);

  const manageModalId = 'insure-manage-modal';

  const openManageModal = (currency: CurrencyBinding) => {
    setSelectedCurrency(currency);
    (document.getElementById(manageModalId) as HTMLDialogElement).showModal();
  };

  const closeManageModal = () => {
    (document.getElementById(manageModalId) as HTMLDialogElement).close();
    setSelectedCurrency(null);
    refetchPools();
  };

  return (
    <>
      <div className="my-14">
        <WalletCard />
        <Card links={links}>
          <div className="px-6 md:px-12 pb-12 pt-4">
            <div className="mb-2">
              <h1 className="text-2xl font-semibold tracking-tight">Insure Pools</h1>
              <p className="text-grey mt-1">
                Provide a safety buffer for lending pools and earn a share of interest income.
              </p>
            </div>

            {/* Desktop column headers */}
            <div className="hidden md:grid md:grid-cols-[80px_1fr_90px_150px_110px_100px_150px_130px] px-1 pb-2 mt-6 border-b border-grey-light">
              <div />
              <div className="text-sm font-semibold text-grey">Asset</div>
              <div className="text-sm font-semibold text-grey">Ticker</div>
              <div className="text-sm font-semibold text-grey">Pool TVL</div>
              <div className="text-sm font-semibold text-grey">APY</div>
              <div className="text-sm font-semibold text-grey">Coverage</div>
              <div className="text-sm font-semibold text-grey">My Position</div>
              <div />
            </div>

            <div>
              {CURRENCY_BINDINGS_ARR.map((currency) => (
                <InsuranceAsset
                  key={currency.ticker}
                  currency={currency}
                  onManageClicked={() => openManageModal(currency)}
                />
              ))}
            </div>

            <StellarExpertLink className="mt-4 text-sm" contractId={contractId} text="View Loan Manager contract" />
          </div>
        </Card>
      </div>
      <InsureManageModal modalId={manageModalId} onClose={closeManageModal} currency={selectedCurrency} />
    </>
  );
};

export default InsurePage;
