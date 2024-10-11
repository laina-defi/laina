import { isNil } from 'ramda';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { Button } from '@components/Button';
import { Loading } from '@components/Loading';
import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import { BINDING_USDC, BINDING_XLM, type CurrencyBinding } from 'src/currency-bindings';
import { useWallet } from 'src/stellar-wallet';
import { BorrowModal } from './BorrowModal';

interface BorrowableAssetCardProps {
  currency: CurrencyBinding;
}

export const BorrowableAsset = ({ currency }: BorrowableAssetCardProps) => {
  const { icon, name, ticker, contractClient } = currency;

  const modalId = `borrow-modal-${ticker}`;

  const { wallet, walletBalances } = useWallet();

  const [totalSupplied, setTotalSupplied] = useState<bigint | null>(null);
  const [totalSuppliedPrice, setTotalSuppliedPrice] = useState<bigint | null>(null);

  // Collateral is the other supported currency for now.
  const collateral = ticker === 'XLM' ? BINDING_USDC : BINDING_XLM;

  const collateralBalance = walletBalances[collateral.ticker];

  const borrowDisabled = !wallet || !collateralBalance || !totalSupplied;

  const fetchAvailableContractBalance = useCallback(async () => {
    if (!contractClient) return;

    try {
      const { result } = await contractClient.get_available_balance();
      setTotalSupplied(result);
    } catch (error) {
      console.error('Error fetching contract data:', error);
    }
  }, [contractClient]); // Dependency on loanPoolContract

  const formatSuppliedAmount = useCallback((amount: bigint | null) => {
    if (amount === BigInt(0)) return '0';
    if (!amount) return <Loading size="xs" />;

    const ten_k = BigInt(10_000 * 10_000_000);
    const one_m = BigInt(1_000_000 * 10_000_000);
    switch (true) {
      case amount > one_m:
        return `${(Number(amount) / (1_000_000 * 10_000_000)).toFixed(2)}M`;
      case amount > ten_k:
        return `${(Number(amount) / (1_000 * 10_000_000)).toFixed(1)}K`;
      default:
        return `${(Number(amount) / 10_000_000).toFixed(1)}`;
    }
  }, []);

  const fetchPriceData = useCallback(async () => {
    if (!loanManagerClient) return;

    try {
      const { result } = await loanManagerClient.get_price({ token: currency.ticker });
      setTotalSuppliedPrice(result);
    } catch (error) {
      console.error('Error fetchin price data:', error);
    }
  }, [currency.ticker]);

  const formatSuppliedAmountPrice = useCallback(
    (price: bigint | null) => {
      if (totalSupplied === BigInt(0)) return '$0';
      if (!totalSupplied || !price) return null;

      const ten_k = BigInt(10_000 * 10_000_000);
      const one_m = BigInt(1_000_000 * 10_000_000);
      const total_price = ((price / BigInt(10_000_000)) * totalSupplied) / BigInt(10_000_000);
      switch (true) {
        case total_price > one_m:
          return `$${(Number(total_price) / (1_000_000 * 10_000_000)).toFixed(2)}M`;
        case total_price > ten_k:
          return `$${(Number(total_price) / (1_000 * 10_000_000)).toFixed(1)}K`;
        default:
          return `$${(Number(total_price) / 10_000_000).toFixed(1)}`;
      }
    },
    [totalSupplied],
  );

  useEffect(() => {
    // Fetch contract data immediately and set an interval to run every 6 seconds
    fetchAvailableContractBalance();
    fetchPriceData();
    const intervalId = setInterval(fetchAvailableContractBalance, 6000);

    // Cleanup function to clear the interval on component unmount
    return () => clearInterval(intervalId);
  }, [fetchAvailableContractBalance, fetchPriceData]); // Now dependent on the memoized function

  const openModal = () => {
    const modalEl = document.getElementById(modalId) as HTMLDialogElement;
    modalEl.showModal();
  };

  const closeModal = () => {
    const modalEl = document.getElementById(modalId) as HTMLDialogElement;
    modalEl.close();
  };

  const tooltip = useMemo(() => {
    if (!totalSupplied) return 'The pool has no assets to borrow';
    if (!wallet) return 'Connect a wallet first';
    if (!collateralBalance) return 'Not enough funds for collateral';
    return 'Something odd happened.';
  }, [totalSupplied, wallet, collateralBalance]);

  return (
    <tr className="border-none text-base h-[6.5rem]">
      <td className="w-20 pl-2 pr-6">
        <img src={icon} alt="" className="mx-auto max-h-12" />
      </td>

      <td>
        <h2 className="font-semibold text-2xl leading-6 mt-3 tracking-tight">{name}</h2>
        <span>{ticker}</span>
      </td>

      <td>
        <p className="text-xl font-semibold leading-6">{formatSuppliedAmount(totalSupplied)}</p>
        <p>{formatSuppliedAmountPrice(totalSuppliedPrice)}</p>
      </td>

      <td>
        <p className="text-xl font-semibold leading-6">1.61%</p>
      </td>

      <td>
        {borrowDisabled ? (
          <div className="tooltip" data-tip={tooltip}>
            <Button disabled={true} onClick={() => {}}>
              Borrow
            </Button>
          </div>
        ) : (
          <Button onClick={openModal}>Borrow</Button>
        )}
      </td>
      {!isNil(totalSupplied) && (
        <BorrowModal
          modalId={modalId}
          onClose={closeModal}
          currency={currency}
          collateral={collateral}
          totalSupplied={totalSupplied}
        />
      )}
    </tr>
  );
};
