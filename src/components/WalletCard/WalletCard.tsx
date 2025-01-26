import AssetsModal from '@components/AssetsModal/AssetsModal';
import { Button } from '@components/Button';
import { Card } from '@components/Card';
import Identicon from '@components/Identicon';
import { Loading } from '@components/Loading';
import LoansModal from '@components/LoansModal/LoansModal';
import { type Loan, useLoans } from '@contexts/loan-context';
import { usePools } from '@contexts/pool-context';
import { type PositionsRecord, type PriceRecord, useWallet } from '@contexts/wallet-context';
import { formatCentAmount, toCents } from '@lib/formatting';
import type { SupportedCurrency } from 'currencies';
import { isNil } from 'ramda';

const ASSET_MODAL_ID = 'assets-modal';
const LOANS_MODAL_ID = 'loans-modal';

const WalletCard = () => {
  const { wallet, openConnectWalletModal, positions } = useWallet();
  const { prices } = usePools();
  const { loans } = useLoans();

  if (!wallet) {
    return (
      <Card bgColor="black" className="text-white p-12 mb-12 min-h-36 flex flex-col justify-center items-start">
        <h2 className="text-xl font-semibold">My Account</h2>
        <p className="mt-2 mb-6">To view your assets, connect a wallet first.</p>
        <Button variant="white" onClick={openConnectWalletModal}>
          Connect Wallet
        </Button>
      </Card>
    );
  }

  const receivables = prices ? calculateTotalReceivables(prices, positions) : null;
  const liabilities = prices && loans ? calculateTotalLiabilities(prices, loans) : null;

  if (isNil(receivables)) {
    return (
      <Card bgColor="black" className="text-white p-12 mb-12 flex flex-row flex-wrap justify-between items-center">
        <div>
          <h2 className="text-xl font-semibold tracking-tight">My Account</h2>
          <div className="my-6 flex flex-row items-center">
            <Identicon address={wallet.address} />
            <div className="ml-9">
              <p className="text-xl">{wallet.displayName}</p>
              <p className="text-grey leading-tight">{wallet.name}</p>
            </div>
          </div>
        </div>
        <span className="flex flex-row items-center">
          <Loading size="lg" className="mr-4" />
          <p>Loading balance</p>
        </span>
      </Card>
    );
  }

  const hasReceivables = receivables > 0n;
  const hasLiabilities = liabilities && liabilities > 0n;

  const openAssetModal = () => {
    const modalEl = document.getElementById(ASSET_MODAL_ID) as HTMLDialogElement;
    modalEl.showModal();
  };

  const closeAssetModal = () => {
    const modalEl = document.getElementById(ASSET_MODAL_ID) as HTMLDialogElement;
    modalEl.close();
  };

  const openLoansModal = () => {
    const modalEl = document.getElementById(LOANS_MODAL_ID) as HTMLDialogElement;
    modalEl.showModal();
  };

  const closeLoansModal = () => {
    const modalEl = document.getElementById(LOANS_MODAL_ID) as HTMLDialogElement;
    modalEl.close();
  };

  return (
    <>
      <Card bgColor="black" className="text-white p-12 mb-12 flex flex-row flex-wrap justify-between">
        <div>
          <h2 className="text-xl font-semibold tracking-tight">My Account</h2>
          <div className="my-6 flex flex-row items-center">
            <Identicon address={wallet.address} />
            <div className="ml-9">
              <p className="text-xl">{wallet.displayName}</p>
              <p className="text-grey leading-tight">{wallet.name}</p>
            </div>
          </div>
        </div>
        <div className="flex flex-col gap-9 my-auto items-end">
          {hasReceivables ? (
            <div className="flex flex-row">
              <div className="w-40 mr-10">
                <p className="text-grey">Total deposited</p>
                <p className="text-xl leading-5">{formatCentAmount(receivables)}</p>
              </div>
              <Button variant="white" className="w-44" onClick={openAssetModal}>
                View Assets
              </Button>
            </div>
          ) : (
            <p>You haven't deposited any assets.</p>
          )}
          {hasLiabilities ? (
            <div className="flex flex-row">
              <div className="w-40 mr-10">
                <p className="text-grey">Total borrowed</p>
                <p className="text-xl leading-5">{formatCentAmount(liabilities)}</p>
              </div>
              <Button variant="white" className="w-44" onClick={openLoansModal}>
                View Loans
              </Button>
            </div>
          ) : (
            <p>You haven't borrowed any assets.</p>
          )}
        </div>
      </Card>
      <AssetsModal modalId={ASSET_MODAL_ID} onClose={closeAssetModal} />
      <LoansModal modalId={LOANS_MODAL_ID} onClose={closeLoansModal} />
    </>
  );
};

const calculateTotalReceivables = (prices: PriceRecord, positions: PositionsRecord): bigint => {
  return Object.entries(positions).reduce((acc, [ticker, { receivable_shares }]) => {
    const price = prices[ticker as SupportedCurrency];
    return acc + toCents(price, receivable_shares);
  }, 0n);
};

const calculateTotalLiabilities = (prices: PriceRecord, loans: Loan[]): bigint => {
  return loans.reduce((acc, loan) => {
    const price = prices[loan.borrowedTicker];
    return acc + toCents(price, loan.borrowedAmount + loan.unpaidInterest);
  }, 0n);
};

export default WalletCard;
