import { FREIGHTER_ID, StellarWalletsKit, WalletNetwork, allowAllModules } from '@creit.tech/stellar-wallets-kit';
import type * as StellarSdk from '@stellar/stellar-sdk';
import { type PropsWithChildren, createContext, useCallback, useContext, useEffect, useState } from 'react';

import { contractClient as loanManagerClient } from '@contracts/loan_manager';
import { getBalances } from '@lib/horizon';
import { CURRENCIES, type SupportedCurrency } from 'currencies';
import { CURRENCY_BINDINGS_ARR } from '../currency-bindings';

const WALLET_TIMEOUT_DAYS = 3;

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
  receivable_shares: bigint;
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
) => Promise<{
  signedTxXdr: XDR_BASE64;
  signerAddress?: string;
}>;

type XDR_BASE64 = string;

const Context = createContext<WalletContext>({
  wallet: null,
  walletBalances: null,
  positions: {},
  openConnectWalletModal: () => {},
  disconnectWallet: () => {},
  refetchBalances: () => {},
  signTransaction: () => Promise.reject(),
});

const kit = new StellarWalletsKit({
  network: import.meta.env.STELLAR_NETWORK === 'local' 
    ? WalletNetwork.LOCAL
    : WalletNetwork.TESTNET,
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

const isSupportedCurrency = (assetCode: string, issuer: string): boolean => {
  return CURRENCIES.some((c) => c.ticker === assetCode && c.issuer === issuer);
};

const createBalanceRecord = (balances: StellarSdk.Horizon.HorizonApi.BalanceLine[]): BalanceRecord =>
  balances.reduce(
    (acc, balanceLine) => {
      if (balanceLine.asset_type === 'native') {
        acc.XLM = { trustLine: true, balanceLine };
      } else if (
        balanceLine.asset_type === 'credit_alphanum4' &&
        isSupportedCurrency(balanceLine.asset_code, balanceLine.asset_issuer)
      ) {
        acc[balanceLine.asset_code as SupportedCurrency] = { trustLine: true, balanceLine };
      }
      return acc;
    },
    {
      XLM: { trustLine: false },
      USDC: { trustLine: false },
      EURC: { trustLine: false },
    } as BalanceRecord,
  );

interface WalletState {
  name: string;
  timeout: Date;
}

const storeWalletState = (state: WalletState) => {
  localStorage.setItem('wallet-state', JSON.stringify(state));
};

const loadWalletState = (): WalletState | null => {
  const text = localStorage.getItem('wallet-state');
  if (!text) return null;

  const { timeout, name } = JSON.parse(text);
  return {
    name,
    timeout: new Date(timeout),
  };
};

const deleteWalletState = () => {
  localStorage.removeItem('wallet-state');
};

export const WalletProvider = ({ children }: PropsWithChildren) => {
  const [wallet, setWallet] = useState<Wallet | null>(null);
  const [walletBalances, setWalletBalances] = useState<BalanceRecord | null>(null);
  const [positions, setPositions] = useState<PositionsRecord>({});

  const loadWallet = useCallback(async (name: string) => {
    try {
      const { address } = await kit.getAddress();
      setWallet(createWalletObj(name, address));
      const balances = await getBalances(address);
      setWalletBalances(createBalanceRecord(balances));
      setPositions(await fetchAllPositions(address));

      // use the user's wallet for all of our contract clients.
      for (const { contractClient } of CURRENCY_BINDINGS_ARR) {
        contractClient.options.publicKey = address;
      }
      loanManagerClient.options.publicKey = address;

      const timeout = new Date();
      timeout.setDate(timeout.getDate() + WALLET_TIMEOUT_DAYS);
      storeWalletState({ name, timeout });
    } catch (err) {
      console.error('Loading wallet failed', err);
      deleteWalletState();
    }
  }, []);

  // Set initial wallet on load.
  useEffect(() => {
    const walletState = loadWalletState();

    if (walletState && new Date().getTime() < walletState.timeout.getTime()) {
      loadWallet(walletState.name);
    }
  }, [loadWallet]);

  const signTransaction: SignTransaction = async (tx, opts) => {
    return kit.signTransaction(tx, opts);
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
    deleteWalletState();
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
