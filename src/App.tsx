import React from 'react';
import { Outlet, RouterProvider, createBrowserRouter } from 'react-router-dom';

import Footer from '@components/Footer';
import Nav from '@components/Nav';
import { LoansProvider } from '@contexts/loan-context';
import { PoolProvider } from '@contexts/pool-context';
import { WalletProvider } from '@contexts/wallet-context';
import BorrowPage from '@pages/_borrow/BorrowPage';
import LandingPage from '@pages/_landing/LandingPage';
import LendPage from '@pages/_lend/LendPage';

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
    ],
  },
]);

const App = () => {
  return (
    <React.StrictMode>
      <WalletProvider>
        <PoolProvider>
          <LoansProvider>
            <RouterProvider router={router} />
          </LoansProvider>
        </PoolProvider>
      </WalletProvider>
    </React.StrictMode>
  );
};

export default App;
