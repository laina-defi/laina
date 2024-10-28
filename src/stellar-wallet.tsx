import { FREIGHTER_ID, StellarWalletsKit, WalletNetwork, allowAllModules } from '@creit.tech/stellar-wallets-kit';
import type * as StellarSdk from '@stellar/stellar-sdk';
import { type PropsWithChildren, createContext, useContext, useEffect, useState } from 'react';

import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import { getBalances } from '@lib/horizon';
import { type SupportedCurrency, isSupportedCurrency } from 'currencies';
import { isNil } from 'ramda';
import { CURRENCY_BINDINGS_ARR } from './currency-bindings';

export type Wallet = {
  name: string;
  address: string;
  displayName: string;
};

export type Balance =
  | { trustLine: false }
  | { trustLine: true; balanceLine: StellarSdk.Horizon.HorizonApi.BalanceLine };

export type BalanceRecord = {
  [K in SupportedCurrency]: Balance;
};

export type Positions = {
  receivables: bigint;
  liabilities: bigint;
  collateral: bigint;
};

export type PositionsRecord = {
  [K in SupportedCurrency]?: Positions;
};

export type PriceRecord = {
  [K in SupportedCurrency]: bigint;
};

export type WalletContext = {
  wallet: Wallet | null;
  walletBalances: BalanceRecord | null;
  positions: PositionsRecord;
  prices: PriceRecord | null;
  openConnectWalletModal: () => void;
  disconnectWallet: () => void;
  refetchBalances: () => void;
  signTransaction: SignTransaction;
};

export type SignTransaction = (
  tx: XDR_BASE64,
  opts?: {
    network?: string;
    networkPassphrase?: string;
    accountToSign?: string;
  },
) => Promise<XDR_BASE64>;

type XDR_BASE64 = string;

const Context = createContext<WalletContext>({
  wallet: null,
  walletBalances: null,
  positions: {},
  prices: null,
  openConnectWalletModal: () => {},
  disconnectWallet: () => {},
  refetchBalances: () => {},
  signTransaction: () => Promise.reject(),
});

const kit: StellarWalletsKit = new StellarWalletsKit({
  network: WalletNetwork.TESTNET,
  selectedWalletId: FREIGHTER_ID,
  modules: allowAllModules(),
});

const createWalletObj = (name: string, address: string): Wallet => ({
  name,
  address,
  displayName: `${address.slice(0, 4)}...${address.slice(-4)}`,
});

const fetchAllPositions = async (user: string): Promise<PositionsRecord> => {
  const positionsArr = await Promise.all(
    CURRENCY_BINDINGS_ARR.map(async ({ contractClient, ticker }) => [
      ticker,
      (await contractClient.get_user_positions({ user })).result,
    ]),
  );
  return Object.fromEntries(positionsArr);
};

const createBalanceRecord = (balances: StellarSdk.Horizon.HorizonApi.BalanceLine[]): BalanceRecord =>
  balances.reduce(
    (acc, balanceLine) => {
      if (balanceLine.asset_type === 'native') {
        acc.XLM = { trustLine: true, balanceLine };
      } else if (balanceLine.asset_type === 'credit_alphanum4' && isSupportedCurrency(balanceLine.asset_code)) {
        acc[balanceLine.asset_code] = { trustLine: true, balanceLine };
      }
      return acc;
    },
    {
      XLM: { trustLine: false },
      wBTC: { trustLine: false },
      wETH: { trustLine: false },
      USDC: { trustLine: false },
      EURC: { trustLine: false },
    } as BalanceRecord,
  );

const fetchAllPrices = async (): Promise<PriceRecord> => {
  const [XLM, wBTC, wETH, USDC, EURC] = await Promise.all([
    fetchPriceData('XLM'),
    fetchPriceData('BTC'),
    fetchPriceData('ETH'),
    fetchPriceData('USDC'),
    fetchPriceData('EURC'),
  ]);
  return { XLM, wBTC, wETH, USDC, EURC };
};

const fetchPriceData = async (token: string): Promise<bigint> => {
  try {
    const { result } = await loanManagerClient.get_price({ token });
    return result;
  } catch (error) {
    console.error(`Error fetching price data: for ${token}`, error);
    return 0n;
  }
};

export const WalletProvider = ({ children }: PropsWithChildren) => {
  const [wallet, setWallet] = useState<Wallet | null>(null);
  const [walletBalances, setWalletBalances] = useState<BalanceRecord | null>(null);
  const [positions, setPositions] = useState<PositionsRecord>({});
  const [prices, setPrices] = useState<PriceRecord | null>(null);

  const loadWallet = async (name: string) => {
    try {
      const { address } = await kit.getAddress();
      setWallet(createWalletObj(name, address));
      const balances = await getBalances(address);
      setWalletBalances(createBalanceRecord(balances));
      setPositions(await fetchAllPositions(address));
      localStorage.setItem('wallet-connected', name);
    } catch (err) {
      console.error('Loading wallet failed', err);
      localStorage.removeItem('wallet-connected');
    }
  };

  // Set initial wallet on load.
  // biome-ignore lint: useEffect is ass
  useEffect(() => {
    const walletConnected = localStorage.getItem('wallet-connected');
    if (!isNil(walletConnected)) {
      loadWallet(walletConnected);
    }
    fetchAllPrices()
      .then((res) => setPrices(res))
      .catch((err) => console.error('Error fetching prices', err));
  }, []);

  const signTransaction: SignTransaction = async (tx, opts) => {
    const { signedTxXdr } = await kit.signTransaction(tx, opts);
    return signedTxXdr;
  };

  const openConnectWalletModal = () => {
    kit.openModal({
      onWalletSelected: ({ name }) => {
        loadWallet(name);
      },
    });
  };

  const disconnectWallet = () => {
    setWallet(null);
    setWalletBalances(null);
    setPositions({});
    localStorage.removeItem('wallet-connected');
  };

  const refetchBalances = async () => {
    if (!wallet) return;

    try {
      const balances = await getBalances(wallet.address);
      setWalletBalances(createBalanceRecord(balances));
      const positions = await fetchAllPositions(wallet.address);
      setPositions(positions);
    } catch (err) {
      console.error('Error fetching balances', err);
    }
  };

  return (
    <Context.Provider
      value={{
        wallet,
        walletBalances,
        positions,
        prices,
        openConnectWalletModal,
        disconnectWallet,
        refetchBalances,
        signTransaction,
      }}
    >
      {children}
    </Context.Provider>
  );
};

export const useWallet = (): WalletContext => useContext(Context);
