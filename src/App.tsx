import React from 'react';
import { Outlet, RouterProvider, createBrowserRouter } from 'react-router-dom';

import Footer from '@components/Footer';
import Nav from '@components/Nav';
import { InsurancePoolProvider } from '@contexts/insurance-pool-context';
import { LoansProvider } from '@contexts/loan-context';
import { PoolProvider } from '@contexts/pool-context';
import { WalletProvider } from '@contexts/wallet-context';
import BorrowPage from '@pages/_borrow/BorrowPage';
import LandingPage from '@pages/_landing/LandingPage';
import LendPage from '@pages/_lend/LendPage';
import InsurePage from '@pages/_insure/InsurePage';

const PageWrapper = () => {
  return (
    <div className="min-h-screen flex flex-col">
      <Nav />
      <main className="max-w-screen flex-1 w-[74rem]">
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
    ],
  },
]);

const App = () => {
  return (
    <React.StrictMode>
      <WalletProvider>
        <PoolProvider>
          <InsurancePoolProvider>
            <LoansProvider>
              <RouterProvider router={router} />
            </LoansProvider>
          </InsurancePoolProvider>
        </PoolProvider>
      </WalletProvider>
    </React.StrictMode>
  );
};

export default App;
