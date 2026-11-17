import { useEffect, useState } from 'react';
import {
  Button, Tabs, TabList, Tab, TabPanel, ToggleButtonGroup, ToggleButton, Switch, Select, SelectValue, Popover,
  ListBox, ListBoxItem, TextField, Input, Label, Tooltip, TooltipTrigger, Focusable,
} from 'react-aria-components';
import { FieldView, type ViewMode } from './FieldView';
import { RunsPanel } from './RunsPanel';
import { TimePanel } from './TimePanel';
import { Icon, Pictogram } from './Pictogram';
import { RUNS, SNAPSHOTS, DT_SNAPSHOT, U_MAX, DOMAIN, SERIES, iterationAt, residualsAt, cdAt, courantAt, STATE_WORD } from './data';
import { VIRIDIS_CSS } from './colormap';

const RUNNING_ID = 'r-20261028T152128-d5cb';
const FAILED_ID = 'r-20261029T230332-0566';

export function RunWorkspace() {
  const [project, setProject] = useState('wind-tunnel');
  const [selected, setSelected] = useState(RUNNING_ID);
  const [available, setAvailable] = useState(62);
  const [snapshot, setSnapshot] = useState(62);
  const [follow, setFollow] = useState(true);
  const [playing, setPlaying] = useState(false);
  const [mode, setMode] = useState<ViewMode>('surface');
  const [meshGrid, setMeshGrid] = useState(false);
  const [viewMoved, setViewMoved] = useState(false);
  const [resetKey, setResetKey] = useState(0);

  // Selecting a run moves the time line to that run's latest snapshot; the running run is followed live.
  const select = (id: string) => {
    setSelected(id);
    setPlaying(false);
    const r = RUNS.find((x) => x.id === id)!;
    setFollow(r.state === 'running');
    setSnapshot(SERIES[id].last(available));
  };

  // The running run produces a new snapshot every 2.5 s (synthetic). Following keeps the scrubber at the head.
  useEffect(() => {
    const id = setInterval(() => setAvailable((a) => Math.min(SNAPSHOTS, a + 1)), 2500);
    return () => clearInterval(id);
  }, []);
  useEffect(() => { if (follow && selected === RUNNING_ID) setSnapshot(available); }, [available, follow, selected]);
  useEffect(() => {
    if (!playing) return;
    const id = setInterval(() => setSnapshot((k) => {
      const end = SERIES[selected].last(available);
      if (k >= end) { setPlaying(false); setFollow(selected === RUNNING_ID); return end; }
      return k + 1;
    }), 120);
    return () => clearInterval(id);
  }, [playing, available, selected]);

  const run = RUNS.find((r) => r.id === selected)!;
  const t = snapshot * DT_SNAPSHOT;
  const res = residualsAt(snapshot);
  const live = { iteration: iterationAt(available), time: available * DT_SNAPSHOT };

  return (
    <div className="app">
      <header className="top">
        <div className="brand">rcfd <span>Studio</span></div>
        <Tabs selectedKey={project} onSelectionChange={(k) => setProject(String(k))} className="projects">
          <TabList aria-label="Projects">
            <Tab id="quick-start">quick-start</Tab>
            <Tab id="wind-tunnel">wind-tunnel</Tab>
            <Tab id="axial-fan">axial-fan</Tab>
          </TabList>
        </Tabs>
        <Button className="ghost new-project"><Icon name="plus" />Open project</Button>
        <div className="top-right">
          <Select aria-label="Solver" defaultSelectedKey="cpu8" className="select on-band">
            <Button className="select-button"><SelectValue /><span className="caret" aria-hidden="true" /></Button>
            <Popover className="popover"><ListBox className="listbox">
              <ListBoxItem id="cpu8">rcfd 0.9 · CPU, 8 threads</ListBoxItem>
              <ListBoxItem id="cpu16">rcfd 0.9 · CPU, 16 threads</ListBoxItem>
            </ListBox></Popover>
          </Select>
          <Button className="primary"><Icon name="plus" />New run</Button>
        </div>
      </header>

      {project !== 'wind-tunnel' ? (
        <main className="other-project">
          <p><b>{project}</b> is not loaded in this prototype. The prototype shows the wind-tunnel project only.</p>
          <Button onPress={() => setProject('wind-tunnel')}>Back to wind-tunnel</Button>
        </main>
      ) : (
        <main className="work">
          <section className="col left" aria-label="Runs">
            <Tabs defaultSelectedKey="runs" className="sub-tabs">
              <TabList aria-label="Project contents">
                <Tab id="runs">Runs</Tab><Tab id="cases">Cases</Tab><Tab id="meshes">Meshes</Tab><Tab id="compare">Compare</Tab>
              </TabList>
              <TabPanel id="runs" className="tab-body">
                <RunsPanel selected={selected} onSelect={select} live={live} />
                <dl className="nums" aria-label={`${run.caseName}, selected run`}>
                  <div className="nums-head">{run.caseName} <span><Pictogram state={run.state} /> {STATE_WORD[run.state]}</span></div>
                  {run.hasField ? (
                    <>
                      <div><dt>Iteration</dt><dd>{iterationAt(snapshot)}</dd></div>
                      <div><dt>Time, s</dt><dd>{t.toFixed(2)}</dd></div>
                      <div><dt>C<sub>d</sub></dt><dd>{cdAt(t).toFixed(3)}</dd></div>
                      <div><dt>res u</dt><dd>{Math.pow(10, res.u).toExponential(1)}</dd></div>
                      <div><dt>Courant max</dt><dd>{courantAt(snapshot).toFixed(2)}</dd></div>
                      <div><dt>Snapshot</dt><dd>{snapshot}/{SNAPSHOTS}</dd></div>
                    </>
                  ) : run.state === 'failed' ? (
                    <>
                      <div><dt>Stopped at</dt><dd>it 211</dd></div>
                      <div><dt>Courant max</dt><dd className="bad">3.4</dd></div>
                      <div><dt>Limit</dt><dd>1.0</dd></div>
                    </>
                  ) : run.state === 'queued' ? (
                    <div className="wide"><dt>Status</dt><dd className="sentence">Starts when a solver slot is free. No numbers yet.</dd></div>
                  ) : (
                    <>
                      <div><dt>Iterations</dt><dd>{run.iterations}</dd></div>
                      <div><dt>Wall time, s</dt><dd>{run.wallS}</dd></div>
                      <div><dt>Ended</dt><dd className="sentence">{run.note}</dd></div>
                    </>
                  )}
                </dl>
              </TabPanel>
              {(['cases', 'meshes', 'compare'] as const).map((k) => (
                <TabPanel key={k} id={k} className="tab-body empty-tab">
                  <p>The {k} view is not part of this prototype. It belongs to a later surface of the Studio roadmap.</p>
                </TabPanel>
              ))}
            </Tabs>
          </section>

          <section className="col mid" aria-label="Field view">
            <div className="view-bar">
              <ToggleButtonGroup aria-label="View" selectionMode="single" disallowEmptySelection selectedKeys={[mode]}
                onSelectionChange={(k) => setMode([...k][0] as ViewMode)} className="seg-group">
                <ToggleButton id="surface">Surface</ToggleButton>
                <ToggleButton id="iso">Iso-lines</ToggleButton>
                <ToggleButton id="streamlines">Streamlines</ToggleButton>
              </ToggleButtonGroup>
              <TooltipTrigger delay={300}>
                <Focusable><span className="disabled-chip" role="button" aria-disabled="true" tabIndex={0}>Cut plane</span></Focusable>
                <Tooltip className="tooltip">Cut planes need a 3D case. cylinder-re100 is 2D.</Tooltip>
              </TooltipTrigger>
              <span className="spacer" />
              <Switch isSelected={meshGrid} onChange={setMeshGrid} className="switch"><span className="track"><span className="knob" /></span>Mesh grid</Switch>
              <Select aria-label="Field" defaultSelectedKey="U" className="select">
                <Button className="select-button"><SelectValue /><span className="caret" aria-hidden="true" /></Button>
                <Popover className="popover"><ListBox className="listbox" disabledKeys={['p']}>
                  <ListBoxItem id="U" textValue="|U| m/s">|U| m/s</ListBoxItem>
                  <ListBoxItem id="p" textValue="p Pa">p Pa, not in the synthetic set</ListBoxItem>
                </ListBox></Popover>
              </Select>
            </div>
            <div className="viewport">
              {run.hasField ? (
                <div className="field-stage">
                  <div className="field-ends" aria-hidden={viewMoved}>
                    <span><Icon name="send" />inlet, 1.0 m/s</span><span>outlet, p = 0</span>
                  </div>
                  <div className="field-frame">
                    <FieldView time={t} mode={mode} meshGrid={meshGrid} resetKey={resetKey} onMoved={() => setViewMoved(true)} />
                    {!viewMoved && (
                      <span className="annot" style={{ left: `${((DOMAIN.CX + 0.02 * DOMAIN.W) / (1.04 * DOMAIN.W)) * 100}%`, top: `${((DOMAIN.H - DOMAIN.CY + 0.06 * DOMAIN.H + DOMAIN.R * 1.6) / (1.12 * DOMAIN.H)) * 100}%` }}>
                        cylinder, D = 0.10 m
                      </span>
                    )}
                    {viewMoved && <Button className="reset-view" onPress={() => { setResetKey((k) => k + 1); setViewMoved(false); }}>Reset view</Button>}
                  </div>
                  <div className="legend-h" aria-label={`Colour map viridis, velocity magnitude 0.00 to ${U_MAX.toFixed(2)} m/s`}>
                    <span className="legend-title">|U|, m/s</span>
                    <span className="legend-scale">
                      <span className="legend-bar" style={{ background: VIRIDIS_CSS.replace('to top', 'to right') }} />
                      <span className="legend-ticks"><span>0.00</span><span>{(U_MAX / 2).toFixed(2)}</span><span>{U_MAX.toFixed(2)}</span></span>
                    </span>
                  </div>
                  <p className="caption">
                    <b>{run.caseName}</b> · |U| velocity magnitude, m/s · t = {t.toFixed(2)} s · it {iterationAt(snapshot)} · viridis ·
                    <span className="synthetic"> synthetic preview data</span>
                  </p>
                </div>
              ) : (
                <div className="no-field">
                  <Pictogram state={run.state} size={28} />
                  {run.state === 'queued' && <p><b>{run.caseName}</b> has not started. Fields appear after its first snapshot.</p>}
                  {run.state === 'failed' && <p><b>{run.caseName}</b> has no saved field: it diverged at iteration {run.iterations}, before its first field write. Its {SERIES[run.id].last(available)} residual and monitor samples are in the time panel below.</p>}
                  {(run.state === 'converged' || run.state === 'finished') && <p>The prototype has no synthetic field for <b>{run.caseName}</b>. Pick cylinder-re100 to see a field.</p>}
                  {run.id !== RUNNING_ID && <Button onPress={() => select(RUNNING_ID)}>Show cylinder-re100</Button>}
                </div>
              )}
            </div>
          </section>

          <aside className="col right" aria-label="Claude Code">
            <div className="chat-head"><span className="dot" aria-hidden="true" />Claude Code <span>· wind-tunnel</span></div>
            <ol className="chat">
              <li className="msg you"><div className="who">You</div>Why did naca0012-aoa4 diverge?</li>
              <li className="msg claude">
                <div className="who">Claude Code</div>
                <p>Residuals grew from iteration 190, and the run stopped at <b>iteration 211</b>. The largest Courant number reached <b>3.4</b> against a limit of 1.0, at the trailing edge, so the time step is too large for that mesh.</p>
                <p>Try dt 5.0e-4 instead of 2.0e-3, or adaptive time stepping.</p>
                <div className="refs">
                  <Button className="ref" onPress={() => select(FAILED_ID)}>r-20261029T230332-0566</Button>
                  <Button className="ref">log.txt, it 211</Button>
                </div>
              </li>
            </ol>
            <form className="chat-input" onSubmit={(e) => e.preventDefault()}>
              <TextField aria-label="Message to Claude Code" className="grow"><Label className="sr-only">Message</Label><Input placeholder="Ask about this project" /></TextField>
              <Button type="submit" className="primary" aria-label="Send"><Icon name="send" />Send</Button>
            </form>
          </aside>
        </main>
      )}

      {project === 'wind-tunnel' && (
        <TimePanel selected={selected} liveIteration={live.iteration} snapshot={snapshot} available={available} onScrub={setSnapshot}
          follow={follow} onFollow={setFollow} playing={playing} onPlay={setPlaying} />
      )}

      <footer className="foot">
        <span>Early-stage software. Not validated for engineering decisions or for safety-critical use.</span>
        <span className="foot-right">Synthetic preview data · design prototype</span>
      </footer>
    </div>
  );
}
