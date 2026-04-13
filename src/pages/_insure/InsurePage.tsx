import { useState } from 'react';

import { Card } from '@components/Card';
import { StellarExpertLink } from '@components/Link';
import { Table } from '@components/Table';
import WalletCard from '@components/WalletCard/WalletCard';
import { useInsurancePools } from '@contexts/insurance-pool-context';
import { contractId } from '@contracts/loan_manager';
import { CURRENCY_BINDINGS_ARR, type CurrencyBinding } from 'src/insurance-bindings';
import { InsuranceAsset } from './InsuranceAsset';
import { InsureManageModal } from './InsureManageModal';
import { InsureMobileCard } from './InsureMobileCard';

const links = [
  { to: '/lend', label: 'Lend' },
  { to: '/borrow', label: 'Borrow' },
  { to: '/insure', label: 'Insure' },
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
          <div className="px-12 pb-12 pt-4">
            <div className="flex items-center justify-between mb-4">
              <h1 className="text-2xl font-semibold tracking-tight">Insure Pools</h1>
            </div>
            <div className="block md:hidden">
              {CURRENCY_BINDINGS_ARR.map((currency) => (
                <InsureMobileCard
                  key={currency.ticker}
                  currency={currency}
                  onManageClicked={() => openManageModal(currency)}
                />
              ))}
            </div>
            <div className="hidden md:block">
              <Table headers={['Asset', null, 'Ticker', 'Pool TVL', 'APY', 'Your Position', null]}>
                {CURRENCY_BINDINGS_ARR.map((currency) => (
                  <InsuranceAsset
                    key={currency.ticker}
                    currency={currency}
                    onManageClicked={() => openManageModal(currency)}
                  />
                ))}
              </Table>
            </div>
            <StellarExpertLink className="mt-3" contractId={contractId} text="View Loan Manager contract" />
          </div>
        </Card>
      </div>
      <InsureManageModal modalId={manageModalId} onClose={closeManageModal} currency={selectedCurrency} />
    </>
  );
};

export default InsurePage;
