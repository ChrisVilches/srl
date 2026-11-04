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

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
);
