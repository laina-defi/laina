import { useState } from 'react';
import { Asset, Horizon, Networks, Operation, TransactionBuilder } from '@stellar/stellar-sdk';

import { Button } from '@components/Button';
import { Dialog, ErrorDialogContent, LoadingDialogContent, SuccessDialogContent } from '@components/Dialog';
import { useWallet } from '@contexts/wallet-context';
import { contractClient as faucetClient } from '@contracts/faucet';
import { sendTransaction } from '@lib/horizon';
import { CURRENCY_EURC, CURRENCY_USDC } from 'currencies';
import EURCIcon from '@images/eurc.svg';
import USDCIcon from '@images/usdc.svg';

const HorizonServer = new Horizon.Server('https://horizon-testnet.stellar.org/');
const LAI_ISSUER = import.meta.env.PUBLIC_ISSUER_ADDRESS_LAI;
const CLAIM_AMOUNT = '10,000';

const FAUCET_TOKENS = [
  { ticker: 'USDC', icon: USDCIcon.src },
  { ticker: 'EURC', icon: EURCIcon.src },
  { ticker: 'LAI',  icon: null },
] as const;

/** Build a single classic tx that adds trustlines for USDC, EURC and LAI. */
const createTrustlinesTx = async (address: string) => {
  const sourceAccount = await HorizonServer.loadAccount(address);
  return new TransactionBuilder(sourceAccount, {
    networkPassphrase: Networks.TESTNET,
    fee: '100000',
  })
    .addOperation(Operation.changeTrust({ asset: new Asset(CURRENCY_USDC.ticker, CURRENCY_USDC.issuer) }))
    .addOperation(Operation.changeTrust({ asset: new Asset(CURRENCY_EURC.ticker, CURRENCY_EURC.issuer) }))
    .addOperation(Operation.changeTrust({ asset: new Asset('LAI', LAI_ISSUER) }))
    .setTimeout(300)
    .build();
};

export interface FaucetModalProps {
  modalId: string;
  onClose: () => void;
}

export const FaucetModal = ({ modalId, onClose }: FaucetModalProps) => {
  const { isSettingUp, isClaiming, isSuccess, error, run, resetState } = useFaucetFlow();

  const closeModal = () => {
    resetState();
    onClose();
  };

  if (isSettingUp) {
    return (
      <Dialog modalId={modalId} onClose={() => { /* prevent close during tx */ }}>
        <LoadingDialogContent title="Setting up token accounts" subtitle="Adding trustlines for USDC, EURC and LAI." onClick={closeModal} />
      </Dialog>
    );
  }

  if (isClaiming) {
    return (
      <Dialog modalId={modalId} onClose={() => { /* prevent close during tx */ }}>
        <LoadingDialogContent title="Claiming tokens" subtitle="Sending 10,000 USDC, EURC and LAI to your wallet." onClick={closeModal} />
      </Dialog>
    );
  }

  if (isSuccess) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <SuccessDialogContent subtitle="You received 10,000 USDC, 10,000 EURC and 10,000 LAI." onClick={closeModal} />
      </Dialog>
    );
  }

  if (error) {
    return (
      <Dialog modalId={modalId} onClose={closeModal}>
        <ErrorDialogContent error={error} onClick={closeModal} />
      </Dialog>
    );
  }

  return (
    <Dialog modalId={modalId} onClose={closeModal}>
      <h3 className="font-bold text-xl mb-2 tracking-tight">Get testnet tokens</h3>
      <p className="text-grey mb-8">Claim tokens to try lending and borrowing on the testnet.</p>

      <ul className="mb-8 flex flex-col gap-3">
        {FAUCET_TOKENS.map(({ ticker, icon }) => (
          <li key={ticker} className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              {icon
                ? <img src={icon} alt={ticker} className="h-9 w-9" />
                : <span className="h-9 w-9 rounded-full bg-black flex items-center justify-center text-white text-sm font-bold">LAI</span>
              }
              <span className="font-semibold text-lg tracking-tight">{ticker}</span>
            </div>
            <span className="font-semibold text-lg">{CLAIM_AMOUNT}</span>
          </li>
        ))}
      </ul>

      <p className="text-grey text-sm mb-8">Your wallet will prompt you twice — once to set up token accounts, once to claim.</p>

      <div className="flex flex-row justify-end gap-4">
        <Button onClick={closeModal} variant="ghost">Cancel</Button>
        <Button onClick={run}>Claim tokens</Button>
      </div>
    </Dialog>
  );
};

const useFaucetFlow = () => {
  const { wallet, signTransaction, refetchBalances } = useWallet();
  const [isSettingUp, setIsSettingUp] = useState(false);
  const [isClaiming, setIsClaiming] = useState(false);
  const [isSuccess, setIsSuccess] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const resetState = () => {
    setIsSettingUp(false);
    setIsClaiming(false);
    setIsSuccess(false);
    setError(null);
  };

  const run = async () => {
    if (!wallet) {
      alert('Please connect your wallet first!');
      return;
    }

    try {
      setIsSettingUp(true);
      const trustTx = await createTrustlinesTx(wallet.address);
      const { signedTxXdr } = await signTransaction(trustTx.toXDR());
      await sendTransaction(signedTxXdr);

      setIsSettingUp(false);
      setIsClaiming(true);
      const claimTx = await faucetClient.claim({ to: wallet.address });
      await claimTx.signAndSend({ signTransaction });

      setIsSuccess(true);
      refetchBalances();
    } catch (err) {
      setError(err as Error);
    } finally {
      setIsSettingUp(false);
      setIsClaiming(false);
    }
  };

  return { isSettingUp, isClaiming, isSuccess, error, run, resetState };
};
