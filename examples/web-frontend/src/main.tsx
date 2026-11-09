import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createBrowserRouter, RouterProvider } from 'react-router';

import { Add } from './Add';
import App, { Study } from './App';
import { Browse } from './Browse';
import './index.css';

// The views, one route each, under App (see src/address.ts). Any other path
// is Study.
const router = createBrowserRouter([
  {
    path: '/',
    element: <App />,
    children: [
      { index: true, element: <Study /> },
      { path: 'browse/:card?/:side?', element: <Browse /> },
      { path: 'add', element: <Add /> },
      { path: '*', element: <Study /> },
    ],
  },
]);

// What the views read from srl, kept by what they asked for (Browse's
// cards, by selector), so that a view shows the answer for what its
// address says, never one that came late for something asked before. It
// is forgotten once no view shows it (gcTime 0): a view shown again reads
// anew (README.md, "Routing the usual way"). srl's answers change only
// when the store does, and an error is srl's, the same when asked again:
// nothing is read again on its own, when the window gets the focus or
// after an error.
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { gcTime: 0, retry: false, refetchOnWindowFocus: false, refetchOnReconnect: false },
  },
});

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
);
