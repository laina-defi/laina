import { useState } from 'react';

import { Button } from '@components/Button';
import { Card } from '@components/Card';
import { StellarExpertLink } from '@components/Link';
import { Table } from '@components/Table';
import WalletCard from '@components/WalletCard/WalletCard';
import { usePools } from '@contexts/pool-context';
import { contractId } from '@contracts/loan_manager';
import { CURRENCY_BINDINGS_ARR, type CurrencyBinding } from 'src/currency-bindings';
import { DepositModal } from './DepositModal';
import { FaucetModal } from './FaucetModal';
import { LendMobileCard } from './LendMobileCard';
import { LendableAsset } from './LendableAsset';

const links = [
  { to: '/lend', label: 'Lend' },
  { to: '/borrow', label: 'Borrow' },
  { to: '/insure', label: 'Insure' },
];

const LendPage = () => {
  const { refetchPools } = usePools();
  const [selectedCurrency, setSelectedCurrency] = useState<CurrencyBinding | null>(null);

  const depositModalId = 'deposit-modal';
  const faucetModalId = 'faucet-modal';

  const openDepositModal = (currency: CurrencyBinding) => {
    setSelectedCurrency(currency);
    (document.getElementById(depositModalId) as HTMLDialogElement).showModal();
  };

  const closeDepositModal = () => {
    (document.getElementById(depositModalId) as HTMLDialogElement).close();
    setSelectedCurrency(null);
    refetchPools();
  };

  const openFaucetModal = () => {
    (document.getElementById(faucetModalId) as HTMLDialogElement).showModal();
  };

  const closeFaucetModal = () => {
    (document.getElementById(faucetModalId) as HTMLDialogElement).close();
  };

  return (
    <>
      <div className="my-14">
        <WalletCard />
        <Card links={links}>
          <div className="px-12 pb-12 pt-4">
            <div className="flex items-center justify-between mb-4">
              <h1 className="text-2xl font-semibold tracking-tight">Lend Assets</h1>
              <Button onClick={openFaucetModal}>Get test tokens</Button>
            </div>
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
      <DepositModal modalId={depositModalId} onClose={closeDepositModal} currency={selectedCurrency} />
      <FaucetModal modalId={faucetModalId} onClose={closeFaucetModal} />
    </>
  );
};

export default LendPage;
