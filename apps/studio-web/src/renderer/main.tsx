// Renderer entry. Browser code only: no Node, no direct file or network access (G10). The design tokens and
// primitives arrive with F-802, the project home with F-805.
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource/archivo/400.css';
import '@fontsource/archivo/700.css';
import './base.css';
import { ProjectViewPlaceholder } from './ProjectViewPlaceholder';

const root = document.getElementById('root');
if (!root) throw new Error('index.html has no #root element');

createRoot(root).render(
  <StrictMode>
    <ProjectViewPlaceholder />
  </StrictMode>,
);
