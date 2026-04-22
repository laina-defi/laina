import { usePools } from '@contexts/pool-context';
import { contractClient as loanManagerClient, contractId as loanManagerContractId } from '@contracts/loan_manager';
import { rpcUrl } from '@contracts/util';
import { rpc, scValToNative } from '@stellar/stellar-sdk';
import type { SupportedCurrency } from 'currencies';
import type { AuctionItem } from 'loan_manager';
import { type PropsWithChildren, createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import { CURRENCY_BINDINGS_BY_ADDRESS, type PoolAddress, isPoolAddress } from 'src/currency-bindings';

export type AuctionWithDetails = {
  auctionItem: AuctionItem;
  borrowedAmount: bigint;
  unpaidInterest: bigint;
  totalDebt: bigint;
  borrowedTicker: SupportedCurrency;
  collateralAmount: bigint;
  collateralShares: bigint;
  collateralTicker: SupportedCurrency;
  borrowerAddress: string;
};

type AuctionsContextType = {
  auctions: AuctionWithDetails[];
  currentLedger: number;
  isLoading: boolean;
  error: string | null;
  refetchAuctions: VoidFunction;
};

const Context = createContext<AuctionsContextType>({
  auctions: [],
  currentLedger: 0,
  isLoading: false,
  error: null,
  refetchAuctions: () => {},
});

const rpcServer = new rpc.Server(rpcUrl, { allowHttp: true });

const auctionKey = (loanId: { borrower_address: string; nonce: bigint | number }) =>
  `${loanId.borrower_address}:${loanId.nonce}`;

export const AuctionsProvider = ({ children }: PropsWithChildren) => {
  const [auctions, setAuctions] = useState<AuctionWithDetails[]>([]);
  const [currentLedger, setCurrentLedger] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { pools } = usePools();
  const poolsRef = useRef(pools);
  poolsRef.current = pools;

  const refetchAuctions = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const { sequence: latestLedger } = await rpcServer.getLatestLedger();
      setCurrentLedger(latestLedger);

      // Auctions last 17280 ledgers (~24h). The testnet RPC has a scan budget per query
      // (~10000 ledgers of dense events), so one call from latestLedger-17280 often returns
      // 0 results. Two parallel calls with staggered startLedgers cover the full window.
      const AUCTION_LEDGER_DURATION = 17_280;
      const [firstHalf, secondHalf] = await Promise.all([
        rpcServer.getEvents({
          startLedger: Math.max(2, latestLedger - AUCTION_LEDGER_DURATION),
          filters: [{ type: 'contract', contractIds: [loanManagerContractId] }],
        }),
        rpcServer.getEvents({
          startLedger: Math.max(2, latestLedger - 9000),
          filters: [{ type: 'contract', contractIds: [loanManagerContractId] }],
        }),
      ]);

      const seenIds = new Set<string>();
      // biome-ignore lint/suspicious/noExplicitAny: SDK event type
      const allEvents = [...firstHalf.events, ...secondHalf.events].filter((e: any) => {
        if (seenIds.has(e.id)) return false;
        seenIds.add(e.id);
        return true;
      });

      // topic[1] is the AuctionItem struct (marked #[topic] in the contract)
      // biome-ignore lint/suspicious/noExplicitAny: SDK event type
      const parseAuction = (event: any): AuctionItem | null => {
        try {
          return scValToNative(event.topic[1]) as AuctionItem;
        } catch {
          return null;
        }
      };

      // biome-ignore lint/suspicious/noExplicitAny: SDK event type
      const getEventName = (event: any): string | null => {
        try {
          return scValToNative(event.topic[0]) as string;
        } catch {
          return null;
        }
      };

      const createdAuctions = allEvents
        .filter((e) => getEventName(e) === 'bad_debt_auction_created')
        .map(parseAuction)
        .filter(Boolean) as AuctionItem[];

      const deletedKeys = new Set(
        allEvents
          .filter((e) => getEventName(e) === 'bad_debt_auction_deleted')
          .map(parseAuction)
          .filter((a): a is AuctionItem => a !== null)
          .map((a) => auctionKey(a.loan_id)),
      );

      const liveAuctions = createdAuctions.filter((a) => !deletedKeys.has(auctionKey(a.loan_id)));

      const currentPools = poolsRef.current;

      const enriched = await Promise.all(
        liveAuctions.map(async (auctionItem): Promise<AuctionWithDetails | null> => {
          try {
            const { result } = await loanManagerClient.get_loan({ loan_id: auctionItem.loan_id });
            if (!result.isOk()) return null;

            const loan = result.unwrap();

            if (!isPoolAddress(loan.borrowed_from) || !isPoolAddress(loan.collateral_from)) return null;

            const borrowedTicker = CURRENCY_BINDINGS_BY_ADDRESS[loan.borrowed_from as PoolAddress].ticker;
            const collateralTicker = CURRENCY_BINDINGS_BY_ADDRESS[loan.collateral_from as PoolAddress].ticker;

            let collateralAmount = loan.collateral_shares;
            if (currentPools) {
              const collateralPool = currentPools[collateralTicker];
              if (collateralPool && collateralPool.totalBalanceShares > 0n) {
                collateralAmount =
                  (loan.collateral_shares * collateralPool.totalBalanceTokens) / collateralPool.totalBalanceShares;
              }
            }

            return {
              auctionItem,
              borrowedAmount: loan.borrowed_amount,
              unpaidInterest: loan.unpaid_interest,
              totalDebt: loan.borrowed_amount + loan.unpaid_interest,
              borrowedTicker,
              collateralAmount,
              collateralShares: loan.collateral_shares,
              collateralTicker,
              borrowerAddress: auctionItem.loan_id.borrower_address,
            };
          } catch (err) {
            return null;
          }
        }),
      );

      setAuctions(enriched.filter(Boolean) as AuctionWithDetails[]);
    } catch (err) {
      setError('Failed to load auctions. Please try again.');
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    refetchAuctions();
    const intervalId = setInterval(refetchAuctions, 30_000);
    return () => clearInterval(intervalId);
  }, [refetchAuctions]);

  return (
    <Context.Provider value={{ auctions, currentLedger, isLoading, error, refetchAuctions }}>
      {children}
    </Context.Provider>
  );
};

export const useAuctions = (): AuctionsContextType => useContext(Context);
