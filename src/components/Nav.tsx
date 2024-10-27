import type { PropsWithChildren } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { useWallet } from 'src/stellar-wallet';
import logo from '/public/laina_v3_shrinked.png';
import { Button } from './Button';
import Identicon from './Identicon';

export default function Nav() {
  const { pathname } = useLocation();
  const { wallet, openConnectWalletModal, disconnectWallet } = useWallet();

  const isIndex = pathname === '/';

  return (
    <nav
      className={`relative mx-auto flex justify-between items-center pt-12 pb-6 px-4 max-w-full ${isIndex ? 'w-[74rem]' : 'w-[64rem]'}`}
    >
      <div>
        <Link to="/">
          <img src={logo.src} alt="logo" className="w-32" />
        </Link>
      </div>

      <div className="hidden md:flex flex-row ml-auto mr-8">
        <LinkItem to="/laina">Laina</LinkItem>
        <LinkItem to="/lend">Assets</LinkItem>
      </div>

      {!wallet ? (
        <Button onClick={openConnectWalletModal}>Connect wallet</Button>
      ) : (
        <div className="dropdown dropdown-end">
          <div tabIndex={0} role="button">
            <Identicon address={wallet.address} />
          </div>
          <ul className="dropdown-content rounded-box bg-white mt-1 mr-1 w-64 z-[1] p-4 shadow">
            <li className="px-8 py-4">
              <p className="font-semibold">{wallet.displayName}</p>
              <p className="text-grey leading-tight">{wallet.name}</p>
            </li>
            <li>
              <Button variant="outline" onClick={disconnectWallet}>
                Disconnect Wallet
              </Button>
            </li>
          </ul>
        </div>
      )}
    </nav>
  );
}

const LinkItem = ({ to, children }: PropsWithChildren<{ to: string }>) => {
  return (
    <Link to={to} className="text-base font-semibold p-4 hover:underline">
      {children}
    </Link>
  );
};
