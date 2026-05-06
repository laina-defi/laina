import { PiArrowSquareOut } from 'react-icons/pi';

export interface LinkProps {
  className?: string;
  text?: string;
  contractId: string;
}

export const StellarExpertLink = ({ className = '', text = 'View contract', contractId }: LinkProps) => {
  const network = import.meta.env.PUBLIC_STELLAR_NETWORK === 'mainnet' ? 'public' : 'testnet';
  const href = `https://stellar.expert/explorer/${network}/contract/${contractId}`;
  return (
    <a
      className={`link flex flex-row items-center gap-0.5 hover:text-grey transition ${className}`}
      href={href}
      target="_blank"
      rel="noreferrer"
    >
      {text}
      <PiArrowSquareOut size="0.9rem" />
    </a>
  );
};
