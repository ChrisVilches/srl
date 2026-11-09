import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createBrowserRouter, RouterProvider } from 'react-router';

import { Add } from './Add';
import { onStoreChange } from './api';
import App, { Study } from './App';
import { Browse } from './Browse';
import './index.css';
import { ONLY_WHEN_ASKED } from './queries';

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

// What the views read from srl (src/queries.ts), kept by what they asked
// for, so that a view shows the answer for what its address says, never
// one that came late for something asked before, and a view shown again
// shows at once what it read last (README.md, "What srl answers"):
// - Each change the page makes to the store (a review, a suspension, an
//   item added or edited) makes everything read out of date but what is
//   read only when asked, such as a card made by its kind program: what is
//   shown is read again at once, the rest when it is next shown.
// - A change made elsewhere, as with srl or an editor, is seen once what
//   was read is STALE_TIME old: when it is next shown, or when the window
//   gets the focus again, as when coming back from the editor.
// - What no view has shown for GC_TIME is forgotten.
// - An error is srl's, the same when asked again: it isn't retried.
const STALE_TIME = 30_000; // ms
const GC_TIME = 10 * 60_000; // ms
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: STALE_TIME, gcTime: GC_TIME, retry: false, refetchOnReconnect: false },
  },
});
onStoreChange(() => queryClient.invalidateQueries({ predicate: (query) => !query.meta?.[ONLY_WHEN_ASKED] }));

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
);
