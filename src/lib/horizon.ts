import { Asset, Horizon, Networks, Operation, type Transaction, TransactionBuilder } from '@stellar/stellar-sdk';
import type { XDR_BASE64 } from '@stellar/stellar-sdk/contract';
import type { Currency } from 'currencies';

const HorizonServer = new Horizon.Server(
  import.meta.env.PUBLIC_STELLAR_NETWORK === 'local' ? 'http://localhost:8000' : 'https://horizon-testnet.stellar.org/',
  { allowHttp: import.meta.env.PUBLIC_STELLAR_NETWORK === 'local' },
);

export const getBalances = async (account: string): Promise<Horizon.HorizonApi.BalanceLine[]> => {
  const { balances } = await HorizonServer.loadAccount(account);
  return balances;
};

export const createAddTrustlineTransaction = async (
  account: string,
  { ticker, issuer }: Currency,
): Promise<Transaction> => {
  const asset = new Asset(ticker, issuer);

  const sourceAccount = await HorizonServer.loadAccount(account);

  const transaction = new TransactionBuilder(sourceAccount, {
    networkPassphrase:
      import.meta.env.PUBLIC_STELLAR_NETWORK === 'local' ? 'Standalone Network ; February 2017' : Networks.TESTNET,
    fee: '100000',
  })
    .addOperation(Operation.changeTrust({ asset }))
    .setTimeout(300) // 5 minutes timeout
    .build();

  return transaction;
};

export const sendTransaction = async (txXdr: XDR_BASE64): Promise<Horizon.HorizonApi.SubmitTransactionResponse> => {
  const tx = TransactionBuilder.fromXDR(
    txXdr,
    import.meta.env.PUBLIC_STELLAR_NETWORK === 'local' ? 'Standalone Network ; February 2017' : Networks.TESTNET,
  );
  return HorizonServer.submitTransaction(tx);
};
