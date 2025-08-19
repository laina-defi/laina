import { Button } from '@components/Button';
import { Loading } from '@components/Loading';
import type { PropsWithChildren } from 'react';
import { FaCircleCheck as CheckMarkIcon, FaCircleXmark as XMarkIcon } from 'react-icons/fa6';
import { parseErrorMessage } from './error-parsing';

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
  // Parse the error message
  const parsedError = parseErrorMessage(error.message);
  console.log(parsedError);

  return (
    <div className="min-w-96 flex flex-col items-center">
      <XMarkIcon className="text-red mb-4" size="2rem" />
      <h3 className="font-bold text-xl mb-4 ">Error</h3>
      <pre>
        <p className="text-sm mb-8 whitespace-pre-wrap">{error.message}</p>
      </pre>
      <Button onClick={onClick}>Close</Button>
    </div>
  );
};
