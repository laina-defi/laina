import React from 'react';
import { Outlet, RouterProvider, createBrowserRouter } from 'react-router-dom';

import Footer from '@components/Footer';
import Nav from '@components/Nav';
import { AuctionsProvider } from '@contexts/auctions-context';
import { InsurancePoolProvider } from '@contexts/insurance-pool-context';
import { LoansProvider } from '@contexts/loan-context';
import { PoolProvider } from '@contexts/pool-context';
import { WalletProvider } from '@contexts/wallet-context';
import AuctionsPage from '@pages/_auctions/AuctionsPage';
import BorrowPage from '@pages/_borrow/BorrowPage';
import InsurePage from '@pages/_insure/InsurePage';
import LandingPage from '@pages/_landing/LandingPage';
import LendPage from '@pages/_lend/LendPage';

const PageWrapper = () => {
  return (
    <div className="min-h-screen flex flex-col">
      <Nav />
      <main className="flex-1 w-[74rem] max-w-full mx-auto px-4 md:px-0">
        <Outlet />
      </main>
      <Footer />
    </div>
  );
};

const router = createBrowserRouter([
  {
    element: <PageWrapper />,
    children: [
      { path: '', element: <LandingPage /> },
      { path: 'lend', element: <LendPage /> },
      { path: 'borrow', element: <BorrowPage /> },
      { path: 'insure', element: <InsurePage /> },
      { path: 'auctions', element: <AuctionsPage /> },
    ],
  },
]);

const App = () => {
  return (
    <React.StrictMode>
      <WalletProvider>
        <PoolProvider>
          <InsurancePoolProvider>
            <AuctionsProvider>
              <LoansProvider>
                <RouterProvider router={router} />
              </LoansProvider>
            </AuctionsProvider>
          </InsurancePoolProvider>
        </PoolProvider>
      </WalletProvider>
    </React.StrictMode>
  );
};

export default App;
