import { Button } from "@components/Button";
import Identicon from "@components/Identicon";
import { useWallet } from "@contexts/wallet-context";
import type { PropsWithChildren } from "react";
import { Link } from "react-router-dom";

export default function Nav() {
	const { wallet, openConnectWalletModal, disconnectWallet } = useWallet();

	return (
		<nav className="relative mx-auto flex justify-between items-center pt-12 pb-6 px-4 max-w-full w-[74rem]">
			<div>
				<Link to="/">
					<img src="laina_v3_shrinked.png" alt="logo" className="w-32" />
				</Link>
			</div>

			<div className="hidden md:flex flex-row ml-auto mr-8">
				<LinkItem to="/">Laina</LinkItem>
				<LinkItem to="/lend">App</LinkItem>
			</div>

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
