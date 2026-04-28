import { isNil } from 'ramda';
import type { ChangeEvent } from 'react';
import { NumericFormat } from 'react-number-format';

import { useWallet } from '@contexts/wallet-context';
import { decimalStringToStroops, isBalanceZero, stroopsToDecimalString } from '@lib/converters';
import { formatCentAmount } from '@lib/formatting';
import type { SupportedCurrency } from 'currencies';
import { Button } from './Button';

export interface CryptoAmountSelectorProps {
  max: bigint;
  value: bigint;
  valueCents: bigint | undefined;
  ticker: SupportedCurrency;
  onChange: (stroops: bigint) => void;
  onSelectMaximum: () => void;
  tickerChangeOptions?: TickerChangeOptions;
}

export interface TickerChangeOptions {
  options: SupportedCurrency[];
  onSelectTicker: (ticker: SupportedCurrency) => void;
}

export const CryptoAmountSelector = ({
  max,
  value,
  valueCents,
  ticker,
  onChange,
  onSelectMaximum,
  tickerChangeOptions,
}: CryptoAmountSelectorProps) => {
  const maxStr = stroopsToDecimalString(max);
  const valueStr = stroopsToDecimalString(value);

  const handleSliderChange = (ev: ChangeEvent<HTMLInputElement>) => {
    onChange(BigInt(ev.target.value));
  };

  const handleChange = (ev: ChangeEvent<HTMLInputElement>) => {
    onChange(decimalStringToStroops(ev.target.value));
  };

  return (
    <>
      <input
        type="range"
        min={0}
        max={max.toString()}
        value={value.toString()}
        className="range"
        onChange={handleSliderChange}
      />
      <div className="flex w-full justify-between px-2 text-xs">
        <span>|</span>
        <span>|</span>
        <span>|</span>
        <span>|</span>
        <span>|</span>
      </div>
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1 max-w-full">
        {isNil(tickerChangeOptions) ? (
          <div className="input input-bordered flex items-center w-full sm:w-64">
            <span className="text-grey w-1/2">{ticker}</span>
            <NumberInput className="w-1/2" value={valueStr} max={maxStr} onChange={handleChange} />
          </div>
        ) : (
          <div className="join w-full sm:w-64">
            <select
              className="select select-bordered w-1/2 join-item"
              value={ticker}
              onChange={(ev) => {
                tickerChangeOptions.onSelectTicker(ev.target.value as SupportedCurrency);
              }}
            >
              {tickerChangeOptions.options.map((ticker) => (
                <TickerOption key={ticker} ticker={ticker} />
              ))}
            </select>
            <NumberInput
              className="input input-bordered flex-1 join-item"
              value={valueStr}
              max={maxStr}
              onChange={handleChange}
            />
          </div>
        )}
        <span className="text-sm">{valueCents ? `≈ ${formatCentAmount(valueCents)}` : null}</span>
        <Button variant="outline" onClick={onSelectMaximum}>
          Select maximum
        </Button>
      </div>
    </>
  );
};

interface NumberInputProps {
  onChange: React.ChangeEventHandler<HTMLInputElement>;
  value: string;
  max: string;
  className?: string;
}

const NumberInput = ({ onChange, value, max, className }: NumberInputProps) => (
  <NumericFormat
    className={className}
    onChange={onChange}
    allowNegative={false}
    decimalScale={7}
    value={value}
    max={max}
  />
);

const TickerOption = ({ ticker }: { ticker: SupportedCurrency }) => {
  const { walletBalances } = useWallet();
  const balance = walletBalances?.[ticker];
  const disabled = isNil(balance) || !balance.trustLine || isBalanceZero(balance.balanceLine.balance);

  return (
    <option disabled={disabled} value={ticker}>
      {ticker}
    </option>
  );
};
