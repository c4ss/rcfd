// The Studio's settings page at #/studio-settings (F-818; the maintainer's answer Q-818-5 of 2026-11-17): the settings
// of the Studio itself, not of a case (those are the case settings, #/settings). Opened by the palette's Settings
// (Ctrl+,) and by the one Settings icon in the header bands. Theme is its first and, for now, only section: no other
// setting is invented here (candidates are listed in ADR 0801 Addendum 17). The setting is the main process's
// (src/main/theme.ts, through the IPC allow-list, G10); a change applies at once. G22: the page's words at rest are its
// name, the section's name and the four choices (word budget `studio-settings`, src/shared/word-budget.ts).
//
// G7 as amended 2026-11-17: the runner section, "Keep the runner going after the Studio closes" (off by default; the
// main process keeps it, through runner.keep.get and runner.keep.set). The budget of 9 words is the page's count before
// it, so the setting is an icon toggle with no word at rest (G22): its name is its accessible name and its tooltip, and
// the section's name is for readers only. With --keep-runner the toggle is on and cannot be turned off here.
import { useEffect, useState } from 'react';
import type { KeepRunnerState } from '../../shared/ipc.js';
import { THEME_LABEL, THEME_SETTINGS, isThemeSetting } from '../../shared/theme.js';
import { IconButton, Quiet, SegmentedToggle } from '../ui/index.js';
import { IconToggle } from '../ui/IconToggle.js';
import { setTheme, useThemeSetting } from '../theme/theme.js';
import './studio-settings.css';

const THEME_OPTIONS = THEME_SETTINGS.map((id) => ({ id, label: THEME_LABEL[id] }));

/** Back to where Settings was opened from; the project home when there is nothing to go back to. */
function leave(): void {
  if (history.length > 1) history.back();
  else location.hash = '';
}

export const KEEP_RUNNER_LABEL = 'Keep the runner going after the Studio closes';

/** The runner setting, as the main process holds it (null until it answers). */
function useKeepRunner(): [KeepRunnerState | null, (keep: boolean) => void] {
  const [state, setState] = useState<KeepRunnerState | null>(null);
  useEffect(() => {
    let live = true;
    void window.studio?.runner
      ?.getKeep()
      .then((s) => live && setState(s))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);
  const set = (keep: boolean) => {
    setState((s) => (s ? { ...s, setting: keep } : s));
    void window.studio?.runner
      ?.setKeep(keep)
      .then(setState)
      .catch(() => undefined);
  };
  return [state, set];
}

export default function StudioSettingsPage() {
  const theme = useThemeSetting();
  const [keep, setKeep] = useKeepRunner();
  return (
    <div className="ss-page">
      <header className="ss-top">
        <div className="ss-brand">
          rcfd <span>Studio</span>
        </div>
        <h1 className="ss-title">Settings</h1>
        <div className="ss-top-right">
          <IconButton variant="ghost" icon="close" label="Close settings" onPress={leave} />
        </div>
      </header>
      <main className="ss-body">
        <section className="ss-section" aria-labelledby="ss-theme">
          <h2 className="ss-section-title" id="ss-theme">
            Theme
          </h2>
          <SegmentedToggle
            aria-label="Theme"
            options={THEME_OPTIONS}
            selected={theme}
            onChange={(id) => {
              if (isThemeSetting(id)) void setTheme(id);
            }}
          />
          {/* G22: what System does is said to readers, not standing on the page */}
          <Quiet>System follows the operating system: dark when it is dark, light otherwise. The field colour maps are the same in every theme.</Quiet>
        </section>
        <section className="ss-section" aria-labelledby="ss-runner">
          <h2 className="sr-only" id="ss-runner">
            Runner
          </h2>
          <IconToggle
            icon="play"
            label={keep?.switch ? `${KEEP_RUNNER_LABEL} (on: the Studio was started with --keep-runner)` : KEEP_RUNNER_LABEL}
            isSelected={keep ? keep.setting || keep.switch : false}
            isDisabled={keep === null || keep.switch}
            onChange={setKeep}
          />
          <Quiet>
            Off: a runner the Studio started is shut down when the Studio closes, after asking about the runs it holds. On: it keeps going, and the next
            start finds it again.
          </Quiet>
        </section>
      </main>
      <footer className="ss-foot">
        <span data-words="disclaimer">
          rcfd is early-stage software. It is not validated for engineering decisions or for safety-critical use, and it
          comes with no warranty.
        </span>
      </footer>
    </div>
  );
}
