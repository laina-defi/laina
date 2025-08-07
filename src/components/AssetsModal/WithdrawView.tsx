import { Button } from "@components/Button";
import { CryptoAmountSelector } from "@components/CryptoAmountSelector";
import {
  ErrorDialogContent,
  LoadingDialogContent,
  SuccessDialogContent,
} from "@components/Dialog";
import { Loading } from "@components/Loading";
import { usePools } from "@contexts/pool-context";
import { useWallet } from "@contexts/wallet-context";
import { stroopsToDecimalString } from "@lib/converters";
import { SCALAR_7, toCents } from "@lib/formatting";
import type { SupportedCurrency } from "currencies";
import { useState } from "react";
import { CURRENCY_BINDINGS } from "src/currency-bindings";

export interface WithdrawViewProps {
  ticker: SupportedCurrency;
  onBack: () => void;
}

const WithdrawView = ({ ticker, onBack }: WithdrawViewProps) => {
  const { name, contractClient } = CURRENCY_BINDINGS[ticker];
  const { positions, wallet, signTransaction } = useWallet();
  const { pools, prices } = usePools();
  const [amount, setAmount] = useState(0n);
  const [isWithdrawing, setIsWithdrawing] = useState(false);
  const [isSuccess, setIsSuccess] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const pool = pools?.[ticker];
  const price = prices?.[ticker];

  const valueCents = price
    ? toCents(price, BigInt(amount) * SCALAR_7)
    : undefined;

  if (!pool) {
    console.warn("PoolState is not loaded");
    return null;
  }

  if (!positions[ticker]) {
    throw Error("Unexpectedly opened WithdrawView without balance");
  }

  const { receivable_shares } = positions[ticker];

  const totalBalance =
    (receivable_shares * pool.totalBalanceTokens) / pool.totalBalanceShares;

  const handleAmountChange = (stroops: bigint) => {
    setAmount(stroops);
  };

  const handleSelectMax = () => {
    setAmount(totalBalance);
  };

  const isWithdrawDisabled = amount === 0n || amount > totalBalance;

  const handleWithdrawClick = async () => {
    if (!wallet) return;

    setIsWithdrawing(true);

    const tx = await contractClient.withdraw({
      user: wallet.address,
      amount,
    });
    try {
      await tx.signAndSend({ signTransaction });
      setIsSuccess(true);
    } catch (err) {
      console.error("Error withdrawing", err);
      setError(err as Error);
    }
    setIsWithdrawing(false);
  };

  if (isWithdrawing) {
    return (
      <LoadingDialogContent
        title="Withdrawing"
        subtitle={`Withdrawing ${stroopsToDecimalString(amount)} ${ticker}.`}
        buttonText="Back"
        onClick={onBack}
      />
    );
  }

  if (isSuccess) {
    return (
      <SuccessDialogContent
        subtitle={`Successfully withdrew ${stroopsToDecimalString(amount)} ${ticker}`}
        buttonText="Back"
        onClick={onBack}
      />
    );
  }

  if (error) {
    return <ErrorDialogContent error={error} onClick={onBack} />;
  }

  return (
    <div className="md:w-[700px]">
      <h3 className="text-xl font-bold tracking-tight mb-8">Withdraw {name}</h3>
      <p className="text-lg mb-2">Select the amount to withdraw</p>
      <CryptoAmountSelector
        max={totalBalance}
        value={amount}
        valueCents={valueCents}
        ticker={ticker}
        onChange={handleAmountChange}
        onSelectMaximum={handleSelectMax}
      />
      <div className="flex flex-row justify-end mt-8">
        <Button onClick={onBack} variant="ghost" className="mr-4">
          Back
        </Button>
        {!isWithdrawing ? (
          <Button disabled={isWithdrawDisabled} onClick={handleWithdrawClick}>
            Withdraw
          </Button>
        ) : (
          <Button disabled>
            <Loading />
            Withdrawing
          </Button>
        )}
      </div>
    </div>
  );
};

export default WithdrawView;
