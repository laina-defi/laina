import { useState } from 'react';

import { Button } from '@components/Button';
import { Card } from '@components/Card';
import { StellarExpertLink } from '@components/Link';
import WalletCard from '@components/WalletCard/WalletCard';
import { usePools } from '@contexts/pool-context';
import { contractId } from '@contracts/loan_manager';
import { CURRENCY_BINDINGS_ARR, type CurrencyBinding } from 'src/currency-bindings';
import { DepositModal } from './DepositModal';
import { FaucetModal } from './FaucetModal';
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
          <div className="px-6 md:px-12 pb-12 pt-4">
            <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3 mb-2">
              <div>
                <h1 className="text-2xl font-semibold tracking-tight">Lend Assets</h1>
                <p className="text-grey mt-1">Supply assets to earn interest.</p>
              </div>
              <Button onClick={openFaucetModal} className="flex-none">Get test tokens</Button>
            </div>

            {/* Desktop column headers */}
            <div className="hidden md:grid md:grid-cols-[80px_1fr_40px_90px_150px_120px_130px_130px] px-1 pb-2 mt-6 border-b border-grey-light">
              <div />
              <div className="text-sm font-semibold text-grey">Asset</div>
              <div />
              <div className="text-sm font-semibold text-grey">Ticker</div>
              <div className="text-sm font-semibold text-grey">Deposits</div>
              <div className="text-sm font-semibold text-grey">Supply APY</div>
              <div className="text-sm font-semibold text-grey">Utilization</div>
              <div />
            </div>

            <div>
              {CURRENCY_BINDINGS_ARR.map((currency) => (
                <LendableAsset
                  key={currency.ticker}
                  currency={currency}
                  onDepositClicked={() => openDepositModal(currency)}
                />
              ))}
            </div>

            <StellarExpertLink className="mt-4 text-sm" contractId={contractId} text="View Loan Manager contract" />
          </div>
        </Card>
      </div>
      <DepositModal modalId={depositModalId} onClose={closeDepositModal} currency={selectedCurrency} />
      <FaucetModal modalId={faucetModalId} onClose={closeFaucetModal} />
    </>
  );
};

export default LendPage;
