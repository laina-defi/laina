import { useState } from 'react';

import { Card } from '@components/Card';
import { StellarExpertLink } from '@components/Link';
import { Table } from '@components/Table';
import WalletCard from '@components/WalletCard/WalletCard';
import { usePools } from '@contexts/pool-context';
import { contractId } from '@contracts/loan_manager';
import { CURRENCY_BINDINGS_ARR, type CurrencyBinding } from 'src/currency-bindings';
import { DepositModal } from './DepositModal';
import { LendMobileCard } from './LendMobileCard';
import { LendableAsset } from './LendableAsset';

const links = [
  { to: '/lend', label: 'Lend' },
  { to: '/borrow', label: 'Borrow' },
];

const LendPage = () => {
  const { refetchPools } = usePools();
  const [selectedCurrency, setSelectedCurrency] = useState<CurrencyBinding | null>(null);

  const modalId = 'deposit-modal';

  const openDepositModal = (currency: CurrencyBinding) => {
    setSelectedCurrency(currency);
    const modalEl = document.getElementById(modalId) as HTMLDialogElement;
    modalEl.showModal();
  };

  const closeDepositModal = () => {
    const modalEl = document.getElementById(modalId) as HTMLDialogElement;
    modalEl.close();
    setSelectedCurrency(null);
    refetchPools();
  };

  return (
    <>
      <div className="my-14">
        <WalletCard />
        <Card links={links}>
          <div className="px-12 pb-12 pt-4">
            <h1 className="text-2xl font-semibold mb-4 tracking-tight">Lend Assets</h1>
            <div className="block md:hidden">
              {CURRENCY_BINDINGS_ARR.map((currency) => (
                <LendMobileCard
                  key={currency.ticker}
                  currency={currency}
                  onDepositClicked={() => openDepositModal(currency)}
                />
              ))}
            </div>
            <div className="hidden md:block">
              <Table headers={['Asset', null, 'Ticker', 'Balance', 'Supply APY', null]}>
                {CURRENCY_BINDINGS_ARR.map((currency) => (
                  <LendableAsset
                    key={currency.ticker}
                    currency={currency}
                    onDepositClicked={() => openDepositModal(currency)}
                  />
                ))}
              </Table>
            </div>
            <StellarExpertLink className="mt-3" contractId={contractId} text="View Loan Manager contract" />
          </div>
        </Card>
      </div>
      <DepositModal modalId={modalId} onClose={closeDepositModal} currency={selectedCurrency} />
    </>
  );
};

export default LendPage;
