import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import AppSatellite from './AppSatellite';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <AppSatellite />
  </StrictMode>,
);
