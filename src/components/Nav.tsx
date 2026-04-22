import { useState } from 'react';

import { Button } from '@components/Button';
import Identicon from '@components/Identicon';
import { useWallet } from '@contexts/wallet-context';
import type { PropsWithChildren } from 'react';
import { PiList, PiX } from 'react-icons/pi';
import { Link, useLocation } from 'react-router-dom';

export default function Nav() {
  const { wallet, openConnectWalletModal, disconnectWallet } = useWallet();
  const [isMenuOpen, setIsMenuOpen] = useState(false);

  return (
    <nav className="relative mx-auto flex flex-wrap justify-between items-center pt-12 pb-6 px-4 max-w-full w-[74rem]">
      <div>
        <Link to="/" onClick={() => setIsMenuOpen(false)}>
          <img src="laina_v3_shrinked.png" alt="logo" className="w-32" />
        </Link>
      </div>

      {/* Desktop nav links */}
      <div className="hidden md:flex flex-row ml-auto mr-8">
        <LinkItem to="/">Laina</LinkItem>
        <LinkItem to="/lend">App</LinkItem>
      </div>

      <div className="flex items-center gap-3">
        {!wallet ? (
          <Button onClick={openConnectWalletModal}>Connect wallet</Button>
        ) : (
          <div className="dropdown dropdown-end">
            <button tabIndex={0} type="button">
              <Identicon address={wallet.address} />
            </button>
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

        {/* Mobile hamburger */}
        <button
          type="button"
          className="md:hidden p-2 rounded hover:bg-grey-lighter transition"
          onClick={() => setIsMenuOpen((v) => !v)}
          aria-label="Toggle menu"
        >
          {isMenuOpen ? <PiX size={24} /> : <PiList size={24} />}
        </button>
      </div>

      {/* Mobile dropdown menu */}
      {isMenuOpen && (
        <div className="w-full md:hidden border-t border-grey-light flex flex-col py-2 mt-2">
          <MobileLinkItem to="/" onClick={() => setIsMenuOpen(false)}>
            Laina
          </MobileLinkItem>
          <MobileLinkItem to="/lend" onClick={() => setIsMenuOpen(false)}>
            App
          </MobileLinkItem>
        </div>
      )}
    </nav>
  );
}

const LinkItem = ({ to, children }: PropsWithChildren<{ to: string }>) => {
  const { pathname } = useLocation();
  const selected = pathname === to;

  return (
    <Link
      to={to}
      className={`relative text-base font-semibold p-4 transition hover:text-black ${selected ? 'text-black' : 'text-grey'}`}
    >
      {children}
      {selected && (
        <span className="absolute bottom-3 transition left-4 right-4 h-0.5 bg-gradient-to-r from-cyan to-magenta rounded-full" />
      )}
    </Link>
  );
};

const MobileLinkItem = ({ to, onClick, children }: PropsWithChildren<{ to: string; onClick: () => void }>) => {
  const { pathname } = useLocation();
  const selected = pathname === to;

  return (
    <Link
      to={to}
      onClick={onClick}
      className={`text-base font-semibold px-4 py-3 rounded transition ${selected ? 'text-black' : 'text-grey hover:text-black'}`}
    >
      {children}
      {selected && (
        <span className="ml-2 inline-block w-1.5 h-1.5 rounded-full bg-gradient-to-r from-cyan to-magenta align-middle" />
      )}
    </Link>
  );
};
