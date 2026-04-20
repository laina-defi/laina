import AssetsModal from '@components/AssetsModal/AssetsModal';
import { Button } from '@components/Button';
import Identicon from '@components/Identicon';
import { Loading } from '@components/Loading';
import LoansModal from '@components/LoansModal/LoansModal';
import { type InsurancePoolRecord, useInsurancePools } from '@contexts/insurance-pool-context';
import { type Loan, useLoans } from '@contexts/loan-context';
import { type PoolRecord, usePools } from '@contexts/pool-context';
import {
  type InsurancePositionsRecord,
  type PositionsRecord,
  type PriceRecord,
  useWallet,
} from '@contexts/wallet-context';
import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import {
  calcBorrowerLaiAPR,
  calcDepositAPY,
  calcInsuranceAPY,
  calcInsurerLaiAPR,
  formatCentAmount,
  toCents,
} from '@lib/formatting';
import type { SupportedCurrency } from 'currencies';
import { isNil } from 'ramda';
import { useEffect, useState } from 'react';
import { PiInfo } from 'react-icons/pi';

const ASSET_MODAL_ID = 'assets-modal';
const LOANS_MODAL_ID = 'loans-modal';

const formatLaiAmount = (amount: number): string => {
  if (amount > 1_000_000) return `${(amount / 1_000_000).toFixed(2)}M`;
  if (amount > 1_000) return `${(amount / 1_000).toFixed(3)}K`;
  return amount.toFixed(4);
};

const WalletCard = () => {
  const {
    wallet,
    openConnectWalletModal,
    positions,
    insurancePositions,
    pendingLai,
    signTransaction,
    refetchBalances,
  } = useWallet();
  const { prices, pools } = usePools();
  const { pools: insurancePools } = useInsurancePools();
  const { loans } = useLoans();
  const [isClaiming, setIsClaiming] = useState(false);
  const [laiAccumulator, setLaiAccumulator] = useState(0);

  const receivables = prices ? calculateTotalReceivables(prices, positions, insurancePositions) : null;
  const liabilities = prices && loans ? calculateTotalLiabilities(prices, loans) : null;
  const netAPYData =
    prices && pools && insurancePools && loans
      ? calculateNetAPY(prices, pools, insurancePools, positions, insurancePositions, loans)
      : null;
  const hasPendingLai = pendingLai !== null && pendingLai > 0n;

  // Reset the accumulator whenever real on-chain data arrives.
  useEffect(() => {
    setLaiAccumulator(0);
  }, []);

  // Tick the display up in real-time based on the calculated earn rate.
  useEffect(() => {
    const laiPerSecond = netAPYData?.laiPerSecond;
    if (!laiPerSecond || !hasPendingLai) return;
    let rafId: number;
    let lastTs: number | null = null;
    const tick = (now: number) => {
      if (lastTs !== null) {
        const delta = now - lastTs;
        setLaiAccumulator((prev) => prev + laiPerSecond * (delta / 1000));
      }
      lastTs = now;
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [netAPYData?.laiPerSecond, hasPendingLai]);

  const displayLai = pendingLai !== null ? Number(pendingLai) / 10_000_000 + laiAccumulator : null;

  const handleClaimLai = async () => {
    if (!wallet) return;
    setIsClaiming(true);
    try {
      const tx = await loanManagerClient.claim_lai_rewards({ user: wallet.address });
      await tx.signAndSend({ signTransaction });
      refetchBalances();
    } catch (err) {
      console.error('Failed to claim LAI rewards', err);
    } finally {
      setIsClaiming(false);
    }
  };

  if (!wallet) {
    return (
      <div className="mb-12 rounded p-[1px] bg-gradient-to-br from-cyan to-magenta shadow">
        <div className="rounded bg-black text-white p-10 min-h-36 flex flex-col justify-center items-start">
          <h2 className="text-xl font-semibold">My Account</h2>
          <p className="mt-2 mb-6 text-grey">Connect a wallet to view your positions and start earning yield.</p>
          <Button variant="white" onClick={openConnectWalletModal}>
            Connect Wallet
          </Button>
        </div>
      </div>
    );
  }

  if (isNil(receivables)) {
    return (
      <div className="mb-12 rounded p-[1px] bg-gradient-to-br from-cyan to-magenta shadow">
        <div className="rounded bg-black text-white p-10 flex flex-row flex-wrap justify-between items-center">
          <div>
            <h2 className="text-xl font-semibold tracking-tight">My Account</h2>
            <div className="my-6 flex flex-row items-center gap-6">
              <Identicon address={wallet.address} />
              <div>
                <p className="text-xl">{wallet.displayName}</p>
                <p className="text-grey leading-tight">{wallet.name}</p>
              </div>
            </div>
          </div>
          <span className="flex flex-row items-center gap-3">
            <Loading size="lg" />
            <p className="text-grey">Loading balance…</p>
          </span>
        </div>
      </div>
    );
  }

  const hasReceivables = receivables > 0n;
  const hasLiabilities = liabilities && liabilities > 0n;

  const openAssetModal = () => (document.getElementById(ASSET_MODAL_ID) as HTMLDialogElement).showModal();
  const closeAssetModal = () => (document.getElementById(ASSET_MODAL_ID) as HTMLDialogElement).close();
  const openLoansModal = () => (document.getElementById(LOANS_MODAL_ID) as HTMLDialogElement).showModal();
  const closeLoansModal = () => (document.getElementById(LOANS_MODAL_ID) as HTMLDialogElement).close();

  return (
    <>
      <div className="mb-12 rounded p-[1px] bg-gradient-to-br from-cyan to-magenta shadow">
        <div className="rounded bg-black text-white p-8 md:p-10">
          <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-8">
            {/* Identity */}
            <div className="flex items-center gap-5">
              <Identicon address={wallet.address} />
              <div>
                <p className="text-sm text-grey mb-1">My Account</p>
                <p className="text-xl font-semibold">{wallet.displayName}</p>
                <p className="text-grey text-sm leading-tight">{wallet.name}</p>
              </div>
            </div>

            {/* Stats grid */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-6 md:gap-10">
              {/* Deposited */}
              <div>
                <p className="text-xs text-grey mb-1">Deposited</p>
                {hasReceivables ? (
                  <>
                    <p className="text-xl font-semibold leading-6">{formatCentAmount(receivables)}</p>
                    <button
                      type="button"
                      onClick={openAssetModal}
                      className="text-xs text-grey hover:text-white underline transition mt-0.5"
                    >
                      View assets
                    </button>
                  </>
                ) : (
                  <p className="text-xl font-semibold leading-6 text-grey">—</p>
                )}
              </div>

              {/* Borrowed */}
              <div>
                <p className="text-xs text-grey mb-1">Borrowed</p>
                {hasLiabilities ? (
                  <>
                    <p className="text-xl font-semibold leading-6">{formatCentAmount(liabilities)}</p>
                    <button
                      type="button"
                      onClick={openLoansModal}
                      className="text-xs text-grey hover:text-white underline transition mt-0.5"
                    >
                      View loans
                    </button>
                  </>
                ) : (
                  <p className="text-xl font-semibold leading-6 text-grey">—</p>
                )}
              </div>

              {/* Net APY */}
              {netAPYData !== null &&
                (() => {
                  const { netAPY, baseAPY, laiBoost } = netAPYData;
                  const tip = `Base yield: ${baseAPY.toFixed(2)}% from interest earnings. Plus ${laiBoost.toFixed(2)}% from LAI token rewards. Net APY: ${netAPY.toFixed(2)}%.`;
                  return (
                    <div>
                      <p className="text-xs text-grey mb-1">Net APY</p>
                      <span className="flex items-center gap-1.5">
                        <span
                          className={`text-xl font-semibold leading-6 ${netAPY >= 0 ? 'text-success' : 'text-error'}`}
                        >
                          {netAPY >= 0 ? '+' : ''}
                          {netAPY.toFixed(2)} %
                        </span>
                        <span className="tooltip tooltip-right cursor-help" data-tip={tip}>
                          <PiInfo size={14} className="opacity-40 hover:opacity-70 transition-opacity" />
                        </span>
                      </span>
                    </div>
                  );
                })()}

              {/* Pending LAI */}
              {hasPendingLai && (
                <div>
                  <p className="text-xs text-grey mb-1">Pending LAI</p>
                  <p className="text-xl font-semibold leading-6 bg-gradient-to-r from-cyan to-magenta bg-clip-text text-transparent tabular-nums">
                    {displayLai !== null ? formatLaiAmount(displayLai) : '—'} LAI
                  </p>
                  <button
                    type="button"
                    onClick={handleClaimLai}
                    disabled={isClaiming}
                    className="text-xs text-grey hover:text-white underline transition mt-0.5 disabled:opacity-50"
                  >
                    {isClaiming ? 'Claiming…' : 'Claim rewards'}
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
      <AssetsModal modalId={ASSET_MODAL_ID} onClose={closeAssetModal} />
      <LoansModal modalId={LOANS_MODAL_ID} onClose={closeLoansModal} />
    </>
  );
};

const calculateTotalReceivables = (
  prices: PriceRecord,
  positions: PositionsRecord,
  insurancePositions: InsurancePositionsRecord,
): bigint => {
  const lending = Object.entries(positions).reduce((acc, [ticker, { receivable_shares }]) => {
    return acc + toCents(prices[ticker as SupportedCurrency], receivable_shares);
  }, 0n);
  const insurance = Object.entries(insurancePositions).reduce((acc, [ticker, amount]) => {
    if (!amount) return acc;
    return acc + toCents(prices[ticker as SupportedCurrency], amount);
  }, 0n);
  return lending + insurance;
};

const calculateTotalLiabilities = (prices: PriceRecord, loans: Loan[]): bigint => {
  return loans.reduce((acc, loan) => {
    const price = prices[loan.borrowedTicker];
    return acc + toCents(price, loan.borrowedAmount + loan.unpaidInterest);
  }, 0n);
};

type NetAPYResult = { netAPY: number; baseAPY: number; laiBoost: number; laiPerSecond: number };

const calculateNetAPY = (
  prices: PriceRecord,
  pools: PoolRecord,
  insurancePools: InsurancePoolRecord,
  positions: PositionsRecord,
  insurancePositions: InsurancePositionsRecord,
  loans: Loan[],
): NetAPYResult | null => {
  let earningsPerYear = 0;
  let laiEarningsPerYear = 0;
  let totalDeposited = 0;
  let totalBorrowed = 0;

  for (const [ticker, pos] of Object.entries(positions)) {
    if (!pos) continue;
    const totalShares = pos.receivable_shares + pos.collateral_shares;
    if (totalShares === 0n) continue;
    const pool = pools[ticker as SupportedCurrency];
    const price = prices[ticker as SupportedCurrency];
    if (!pool || !price) continue;
    const valueInCents = Number(toCents(price, totalShares));
    const apy = calcDepositAPY(pool.annualInterestRate, pool.totalBalanceTokens, pool.availableBalanceTokens);
    totalDeposited += valueInCents;
    earningsPerYear += valueInCents * apy;
  }

  for (const [ticker, amount] of Object.entries(insurancePositions)) {
    if (!amount || amount === 0n) continue;
    const pool = pools[ticker as SupportedCurrency];
    const insurancePool = insurancePools[ticker as SupportedCurrency];
    const price = prices[ticker as SupportedCurrency];
    if (!pool || !insurancePool || !price) continue;
    const valueInCents = Number(toCents(price, amount));
    const baseAPY = calcInsuranceAPY(
      pool.annualInterestRate,
      pool.totalBalanceTokens,
      pool.availableBalanceTokens,
      insurancePool.totalTokens,
    );
    const laiAPR = calcInsurerLaiAPR(insurancePool.totalTokens, price);
    totalDeposited += valueInCents;
    earningsPerYear += valueInCents * (baseAPY + laiAPR);
    laiEarningsPerYear += valueInCents * laiAPR;
  }

  for (const loan of loans) {
    const pool = pools[loan.borrowedTicker];
    const price = prices[loan.borrowedTicker];
    if (!pool || !price) continue;
    const valueInCents = Number(toCents(price, loan.borrowedAmount));
    const apr = Number(pool.annualInterestRate) / 100_000;
    const laiAPR = calcBorrowerLaiAPR(pool.totalBalanceTokens, pool.availableBalanceTokens, price);
    totalBorrowed += valueInCents;
    earningsPerYear -= valueInCents * (apr - laiAPR);
    laiEarningsPerYear += valueInCents * laiAPR;
  }

  const denominator = totalDeposited > 0 ? totalDeposited : totalBorrowed;
  if (denominator === 0) return null;
  const netAPY = earningsPerYear / denominator;
  const laiBoost = laiEarningsPerYear / denominator;
  // laiEarningsPerYear is in (cents × percentage-points); /10000 → USD/year; /LAI_PRICE_USD → LAI/year; /SECONDS_PER_YEAR → LAI/s
  const laiPerSecond = laiEarningsPerYear / 10_000 / 0.01 / (365 * 24 * 3600);
  return { netAPY, baseAPY: netAPY - laiBoost, laiBoost, laiPerSecond };
};

export default WalletCard;
