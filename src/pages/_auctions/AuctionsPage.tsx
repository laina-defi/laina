import { useState } from 'react';

import { Card } from '@components/Card';
import { Loading } from '@components/Loading';
import WalletCard from '@components/WalletCard/WalletCard';
import { type AuctionWithDetails, useAuctions } from '@contexts/auctions-context';
import { AuctionCard } from './AuctionCard';
import { ClaimModal } from './ClaimModal';

const links = [
  { to: '/lend', label: 'Lend' },
  { to: '/borrow', label: 'Borrow' },
  { to: '/insure', label: 'Insure' },
  { to: '/auctions', label: 'Auctions' },
];

const claimModalId = 'claim-auction-modal';

const AuctionsPage = () => {
  const { auctions, currentLedger, isLoading, error } = useAuctions();
  const [selectedAuction, setSelectedAuction] = useState<AuctionWithDetails | null>(null);

  const openClaimModal = (auction: AuctionWithDetails) => {
    setSelectedAuction(auction);
    (document.getElementById(claimModalId) as HTMLDialogElement).showModal();
  };

  const closeClaimModal = () => {
    (document.getElementById(claimModalId) as HTMLDialogElement).close();
    setSelectedAuction(null);
  };

  return (
    <>
      <div className="my-14">
        <WalletCard />
        <Card links={links}>
          <div className="px-6 md:px-12 pb-12 pt-4">
            <div className="mb-2">
              <h1 className="text-2xl font-semibold tracking-tight">Bad Debt Auctions</h1>
              <p className="text-grey mt-1">
                Claim undercollateralised loans. Payment required decays to zero over 24 hours — you always receive all
                collateral.
              </p>
            </div>

            {/* Desktop column headers */}
            <div className="hidden md:grid md:grid-cols-[200px_1fr_1fr_1fr_160px_130px_40px] px-1 pb-2 mt-6 border-b border-grey-light">
              <div className="text-sm font-semibold text-grey">Borrower</div>
              <div className="text-sm font-semibold text-grey">Total debt</div>
              <div className="text-sm font-semibold text-grey">Collateral</div>
              <div className="text-sm font-semibold text-grey">Pay now</div>
              <div className="text-sm font-semibold text-grey">P/L now</div>
              <div />
              <div />
            </div>

            {isLoading && auctions.length === 0 && (
              <div className="flex justify-center py-12">
                <Loading size="lg" />
              </div>
            )}

            {error && (
              <div className="alert alert-error mt-6">
                <span>{error}</span>
              </div>
            )}

            {!isLoading && !error && auctions.length === 0 && (
              <div className="text-center py-12 text-grey">
                <p className="text-lg font-semibold">No active auctions</p>
                <p className="text-sm mt-1">
                  Bad debt auctions will appear here when loans become undercollateralised.
                </p>
              </div>
            )}

            <div>
              {auctions.map((auction) => (
                <AuctionCard
                  key={`${auction.borrowerAddress}:${auction.auctionItem.loan_id.nonce}`}
                  auction={auction}
                  currentLedger={currentLedger}
                  onClaimClicked={() => openClaimModal(auction)}
                />
              ))}
            </div>
          </div>
        </Card>
      </div>

      <ClaimModal
        modalId={claimModalId}
        onClose={closeClaimModal}
        auction={selectedAuction}
        currentLedger={currentLedger}
      />
    </>
  );
};

export default AuctionsPage;
