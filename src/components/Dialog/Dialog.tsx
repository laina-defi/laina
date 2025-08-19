import { Button } from '@components/Button';
import { Loading } from '@components/Loading';
import { contractId as loanManagerContractId } from '@contracts/loan_manager';
import { contractId as poolEurcContractId } from '@contracts/pool_eurc';
import { contractId as poolUsdcContractId } from '@contracts/pool_usdc';
import { contractId as poolXlmContractId } from '@contracts/pool_xlm';
import type { PropsWithChildren } from 'react';
import { FaCircleCheck as CheckMarkIcon, FaCircleInfo as InfoIcon, FaCircleXmark as XMarkIcon } from 'react-icons/fa6';
import { BINDING_EURC, BINDING_USDC, BINDING_XLM } from 'src/currency-bindings';
import { type EventType, parseErrorMessage } from './error-parsing';

export interface DialogProps {
  modalId: string;
  onClose: VoidFunction;
  className?: string;
}

export const Dialog = ({ modalId, onClose, children, className = '' }: PropsWithChildren<DialogProps>) => (
  <dialog id={modalId} className="modal">
    <div className={`modal-box p-10 flex flex-col w-auto max-w-screen-2xl ${className}`}>{children}</div>
    {/* Invisible backdrop that closes the modal on click */}
    <form method="dialog" className="modal-backdrop">
      <button onClick={onClose} type="button">
        close
      </button>
    </form>
  </dialog>
);

export interface DialogContentProps {
  title?: string;
  subtitle?: string;
  onClick: VoidFunction;
  buttonText?: string;
}

export const LoadingDialogContent = ({ title = 'Loading', subtitle, buttonText = 'Close' }: DialogContentProps) => (
  <div className="w-96 flex flex-col items-center">
    <Loading size="lg" className="mb-4" />
    <h3 className="font-bold text-xl mb-4">{title}</h3>
    {subtitle ? <p className="text-lg mb-8">{subtitle}</p> : null}
    <Button disabled={true}>{buttonText}</Button>
  </div>
);

export const SuccessDialogContent = ({
  title = 'Success',
  subtitle,
  onClick,
  buttonText = 'Close',
}: DialogContentProps) => (
  <div className="w-96 flex flex-col items-center">
    <CheckMarkIcon className="text-green mb-4" size="2rem" />
    <h3 className="font-bold text-xl mb-4">{title}</h3>
    {subtitle ? <p className="text-lg mb-8">{subtitle}</p> : null}
    <Button onClick={onClick}>{buttonText}</Button>
  </div>
);

export const ErrorDialogContent = ({ error, onClick }: { error: Error; onClick: VoidFunction }) => {
  const parsedError = parseErrorMessage(error.message);

  return (
    <div className="min-w-96 flex flex-col items-center">
      <h3 className="font-bold text-xl mb-4 w-full">
        <XMarkIcon className="text-red mb-1 mr-2 inline-block" size="2rem" />
        {parsedError.mainError}
      </h3>
      <div className="w-full">
        {parsedError.eventLog.map((event) => (
          <pre key={event.index} className="my-2">
            <h4 className="font-bold">
              {eventTypeToIcon(event.type)} {event.index}: {contractIdToName(event.contract)}{' '}
              <span className="font-normal">– {event.type}</span>
            </h4>
            <p className="text-sm whitespace-pre-wrap">{event.topics.join(', ')}</p>
            <p className="text-sm whitespace-pre-wrap">{event.data}</p>
          </pre>
        ))}
      </div>
      <Button onClick={onClick}>Close</Button>
    </div>
  );
};

const eventTypeToIcon = (type: EventType) => {
  switch (type) {
    case 'Diagnostic Event':
      return <InfoIcon className="text-blue inline-block mb-1" size="1rem" />;
    case 'Contract Event':
      return <CheckMarkIcon className="text-green inline-block mb-1" size="1rem" />;
    case 'Failed Diagnostic Event (not emitted)':
      return <XMarkIcon className="text-red inline-block mb-1" size="1rem" />;
    case 'Failed Contract Event (not emitted)':
      return <XMarkIcon className="text-red inline-block mb-1" size="1rem" />;
  }
};

const contractIdToName = (contractId: string | undefined) => {
  if (!contractId) return 'Unknown';
  switch (contractId) {
    case loanManagerContractId:
      return 'Loan Manager';
    case poolXlmContractId:
      return 'XLM Pool';
    case poolUsdcContractId:
      return 'USDC Pool';
    case poolEurcContractId:
      return 'EURC Pool';
    // Testnet Reflector address.
    case 'CCYOZJCOPG34LLQQ7N24YXBM7LL62R7ONMZ3G6WZAAYPB5OYKOMJRN63':
      return 'Oracle';
    case BINDING_XLM.tokenContractAddress:
      return 'XLM Asset Contract';
    case BINDING_USDC.tokenContractAddress:
      return 'USDC Asset Contract';
    case BINDING_EURC.tokenContractAddress:
      return 'EURC Asset Contract';
    default:
      return contractId;
  }
};
