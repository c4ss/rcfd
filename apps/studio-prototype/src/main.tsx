import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource/archivo/400.css';
import '@fontsource/archivo/500.css';
import '@fontsource/archivo/700.css';
import './styles.css';
import { RunWorkspace } from './RunWorkspace';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RunWorkspace />
  </StrictMode>,
);
