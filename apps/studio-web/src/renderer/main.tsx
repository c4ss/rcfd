// Renderer entry. Browser code only: no Node, no direct file or network access (G10).
//
// Routes (the location hash; navigation stays on app://studio/index.html):
//   (none)             the project home (F-805): no project open, or the open project folder
//   #/dev/primitives   the primitives gallery (F-802), for development and the maintainer's view-and-report
//                      (npm run gallery); loaded on demand, so it costs the project home nothing
//   #/settings[/<id>]  the case settings of a service session (F-825), loaded on demand
//   #/dev/plots        the residual and monitor plots of a service session (F-811), loaded on demand
//   #/dev/field?...    the field viewport on a store (F-808, dev/FieldDev.tsx; npm run field), loaded on demand
//   #/case[/...]       the case editor (F-812): open, edit, check and save a case file of the project; loaded on demand
import { lazy, StrictMode, Suspense, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource/archivo/400.css';
import '@fontsource/archivo/500.css';
import '@fontsource/archivo/700.css';
import './design/tokens.css';
import './base.css';
import './ui/primitives.css';
import { ProjectHome } from './project/ProjectHome';
// F-831: the command palette and the global shortcuts, beside every route (the palette loads on first use)
import { PaletteHost } from './commands/PaletteHost';
// F-837: save or discard before a close that would lose case changes (the window's close request included)
import { CloseGuardHost } from './case/closeGuard';
// F-818: the theme setting, drawn as soon as the main process answers (tokens.css follows the system until then)
import { initTheme } from './theme/theme';

initTheme();

const GALLERY_HASH = '#/dev/primitives';

const PrimitivesGallery = lazy(() => import('./dev/PrimitivesGallery'));
const PLOTS_HASH = '#/dev/plots';
const PlotsDev = lazy(() => import('./dev/PlotsDev'));

const SETTINGS_HASH = '#/settings';
const SettingsPage = lazy(() => import('./settings/SettingsPage'));

const FIELD_HASH = '#/dev/field';
const FieldDev = lazy(() => import('./dev/FieldDev'));
const CASE_HASH = '#/case';
const CaseEditorPage = lazy(() => import('./case/CaseEditorPage'));

// React Aria's usePress adds a one-rule <style> element unless an element with this id exists; the Content Security
// Policy refuses inline style elements, so the rule is in base.css and this marker stands in for the element.
const PRESSABLE_STYLE_ID = 'react-aria-pressable-style';
if (!document.getElementById(PRESSABLE_STYLE_ID)) {
  const marker = document.createElement('meta');
  marker.id = PRESSABLE_STYLE_ID;
  marker.name = 'react-aria-pressable-style';
  marker.content = 'in base.css';
  document.head.append(marker);
}

function Root() {
  const [hash, setHash] = useState(() => location.hash);
  useEffect(() => {
    const onHash = () => setHash(location.hash);
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);
  if (hash === GALLERY_HASH) {
    return (
      <Suspense fallback={null}>
        <PrimitivesGallery />
      </Suspense>
    );
  }
  if (hash === PLOTS_HASH || hash.startsWith(`${PLOTS_HASH}?`)) {
    return (
      <Suspense fallback={null}>
        <PlotsDev />
      </Suspense>
    );
  }
  if (hash === SETTINGS_HASH || hash.startsWith(`${SETTINGS_HASH}/`)) {
    return (
      <Suspense fallback={null}>
        <SettingsPage />
      </Suspense>
    );
  }
  if (hash === FIELD_HASH || hash.startsWith(`${FIELD_HASH}?`)) {
    return (
      <Suspense fallback={null}>
        <FieldDev hash={hash} />
      </Suspense>
    );
  }
  if (hash === CASE_HASH || hash.startsWith(`${CASE_HASH}/`)) {
    return (
      <Suspense fallback={null}>
        <CaseEditorPage />
      </Suspense>
    );
  }
  return <ProjectHome />;
}

const root = document.getElementById('root');
if (!root) throw new Error('index.html has no #root element');

createRoot(root).render(
  <StrictMode>
    <Root />
    <PaletteHost />
    <CloseGuardHost />
  </StrictMode>,
);
