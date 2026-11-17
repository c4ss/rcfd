// Run sites in the Studio (F-848, ADR 0739 phase 1; docs/studio-guardrails.md G10 as amended 2026-11-17, G12, G16,
// G18, G22): the user's sites file as the engine lists it, the Where choice at Start, the confirmation of a site, and
// the link state of a remote run. Shared by the renderer, the main process and the tests: no Node or DOM types.
//
// - The sites are the engine's: `rcfd site list --json` and `rcfd site check <name> [--confirm]` (crates/rcfd-runner
//   src/site/cmd.rs). The Studio reads the answers and never parses or writes sites.toml itself; Open the sites file
//   opens it in the system's editor (no editor here).
// - A remote run starts as `rcfd-runner start <case> --set ... --site <name> --no-runner --project <folder>` (the
//   action below), not MCP start_run {site}: the engine opens start_run's site only to sites with assistant = true,
//   because it cannot tell the Studio from Claude Code (ADR 0739 "Built"). The settings of the store plan that the
//   Studio's Run asks for (ADR 0737) come from start_run's dry run, so a remote run stores what a local one would.
// - While the sites file is absent no engine program starts, nothing is written, and the Studio shows no new word
//   (G22): the Where choice needs a listed site, and a badge a run whose run.json names a site.
// - The Where choice is remembered per project in the Studio's user data (site-choices.json beside the recent list),
//   never in the project (Q8): the project holds no host, no site and nothing machine-specific.
// - The link state of a remote run comes from what the local runner wrote into the run folder (site.json, site.jsonl,
//   and the events it copied), read in the main process; nothing in the Studio talks to a site (P4). run.json's site is
//   read whether or not the sites file is there, so a run whose site was removed keeps its badge (P7).
import type { ParamsSpec } from './actions/params.js';
import type { RegisteredRunAction, RunActionContext, RunActionId, RunActionSpec } from './actions/runs.js';

/** The local machine, always there: today's runs. */
export const LOCAL = 'local';

/** A site name as the engine takes it (sites.rs valid_name): letters, digits, _ and -, at most 64. */
export const isSiteName = (s: unknown): s is string => typeof s === 'string' && /^[A-Za-z0-9_-]{1,64}$/.test(s);

/** One site of `rcfd site list --json` (SiteEntry::listing). */
export interface SiteListing {
  readonly name: string;
  readonly kind: string;
  readonly host: string | null;
  readonly engine: string;
  readonly work: string;
  readonly assistant: boolean;
  readonly confirmed: boolean;
  readonly acceptEngines: readonly string[];
}

/** What the `sites` list answers: the file, whether it is there, its sites, and the open project's remembered choice. */
export interface SitesState {
  /** The sites file the engine reads (RCFD_SITES_FILE, else the user's config folder); null when there is none. */
  readonly file: string | null;
  readonly exists: boolean;
  readonly sites: readonly SiteListing[];
  /** The engine's refusal of the file (a key it does not know, a table that does not parse), with the file named. */
  readonly error: string | null;
  /** The open project's last Where (a site of the list, or LOCAL); LOCAL with no project or no choice. */
  readonly choice: string;
}

export const NO_SITES: SitesState = { file: null, exists: false, sites: [], error: null, choice: LOCAL };

/** What `rcfd site check <name> [--confirm]` answered (cmd.rs check). */
export interface SiteCheck {
  readonly site: string;
  readonly host: string | null;
  readonly kind: string;
  /** The host key fingerprints OpenSSH holds (exec for a test site), or why there are none. */
  readonly fingerprint: string | null;
  readonly fingerprintError: string | null;
  readonly engine: string;
  readonly localEngine: string;
  readonly sameEngine: boolean;
  readonly protocolOk: boolean;
  readonly confirmed: boolean;
  /** Whether a run may go to the site now. */
  readonly admitted: boolean;
  /** Why not, in the engine's words. */
  readonly reason: string | null;
}

/** The answer of a check or a confirm: the report, or the engine's message when it could not check. */
export type SiteCheckResult = { readonly ok: true; readonly check: SiteCheck } | { readonly ok: false; readonly message: string };

/** What the `sites` operation of the project port does. */
export const SITE_OPS = ['list', 'check', 'confirm', 'open'] as const;
export type SiteOp = (typeof SITE_OPS)[number];

type Obj = Record<string, unknown>;
const obj = (v: unknown): Obj => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Obj) : {});
const str = (v: unknown, d = ''): string => (typeof v === 'string' ? v : d);
const strOrNull = (v: unknown): string | null => (typeof v === 'string' ? v : null);

/** The sites of `rcfd site list --json`; entries without a valid name are dropped. */
export function parseSiteList(v: unknown): SiteListing[] {
  const out: SiteListing[] = [];
  for (const x of (obj(v)['sites'] as unknown[] | undefined) ?? []) {
    const o = obj(x);
    if (!isSiteName(o['name']) || o['name'] === LOCAL) continue;
    out.push({
      name: o['name'],
      kind: str(o['kind']),
      host: strOrNull(o['host']),
      engine: str(o['engine']),
      work: str(o['work']),
      assistant: o['assistant'] === true,
      confirmed: o['confirmed'] === true,
      acceptEngines: ((o['accept_engines'] as unknown[] | undefined) ?? []).filter((e): e is string => typeof e === 'string'),
    });
  }
  return out;
}

/** The report of `rcfd site check`. */
export function parseSiteCheck(v: unknown): SiteCheck {
  const o = obj(v);
  return {
    site: str(o['site']),
    host: strOrNull(o['host']),
    kind: str(o['kind']),
    fingerprint: strOrNull(o['fingerprint']),
    fingerprintError: strOrNull(o['fingerprint_error']),
    engine: str(o['engine']),
    localEngine: str(o['local_engine']),
    sameEngine: o['same_engine'] === true,
    protocolOk: o['protocol_ok'] === true,
    confirmed: typeof o['confirmed'] === 'string' && o['confirmed'] !== '',
    admitted: o['admitted'] === true,
    reason: strOrNull(o['reason']),
  };
}

/** The Where of the run bar: the remembered choice when that site is still listed, else LOCAL. */
export function whereOf(state: SitesState): string {
  return state.sites.some((s) => s.name === state.choice) ? state.choice : LOCAL;
}

// ---- the link state of a remote run (P6)

/**
 * sending: queued here, its inputs not yet on the site; connected: the link is up and the run's files are copied as
 * they grow; retrying: the link was lost (or the site is not in the sites file now) and the runner tries again;
 * copying: the run ended on the site and its outputs and fields are being copied back (Q4); done: everything is here;
 * refused: the site took no run (its engine or protocol, or an input that cannot go), the run failed here.
 */
export type SiteLink = 'sending' | 'connected' | 'retrying' | 'copying' | 'done' | 'refused';

/** The link state in words (G22: shown beside the site's name, on remote runs only). */
export const LINK_WORDS: Readonly<Record<SiteLink, string>> = {
  sending: 'sending',
  connected: 'connected',
  retrying: 'retrying',
  copying: 'copying back',
  done: 'done',
  refused: 'refused',
};

/** A remote run as the run list and the workspace show it. */
export interface RunSite {
  readonly site: string;
  readonly link: SiteLink;
  /** The link's last message (why it was lost or refused), when it gave one. */
  readonly message: string | null;
  /** When the link was last seen working (site.json last_contact), RFC 3339. */
  readonly lastContact: string | null;
}

/** What the main process read of one remote run's folder. */
export interface RunSiteFiles {
  /** run.json `site`. */
  readonly site: string;
  /** site.json, parsed (null when the runner has not written it yet). */
  readonly siteJson: Readonly<Record<string, unknown>> | null;
  /** The lines of site.jsonl, parsed, oldest first (a line that does not parse is left out). */
  readonly events: readonly Readonly<Record<string, unknown>>[];
  /** Whether the run's copied events hold a state change to finished, failed or cancelled. */
  readonly endedOnSite: boolean;
  /** Whether the run's own status.json is finished, failed or cancelled. */
  readonly endedHere: boolean;
}

const LINK_KINDS = new Set(['connected', 'lost', 'unconfigured', 'refused']);

/** The link state of a remote run from its files (site.json, site.jsonl and its copied events). */
export function runSiteOf(f: RunSiteFiles): RunSite {
  const sj = f.siteJson ?? {};
  const lastContact = strOrNull(sj['last_contact']);
  const last = [...f.events].reverse().find((e) => LINK_KINDS.has(str(e['kind'])));
  const message = last && str(last['kind']) !== 'connected' ? strOrNull(last['message']) : null;
  const at = (link: SiteLink, m: string | null = message): RunSite => ({ site: f.site, link, message: m, lastContact });
  if (sj['link'] === 'done' || sj['done'] === true) return at('done', null);
  const kind = last ? str(last['kind']) : null;
  if (kind === 'refused') return at(f.endedHere ? 'refused' : 'retrying');
  if (kind === 'lost' || kind === 'unconfigured') return at('retrying');
  if (sj['submitted'] !== true) return at('sending');
  if (f.endedOnSite && !f.endedHere) return at('copying', null);
  return at('connected', null);
}

/** Whether the link of a remote run can still change (the main process stops reading a run's files once it is done). */
export const linkSettled = (r: RunSite): boolean => r.link === 'done' || r.link === 'refused';

// ---- the action that starts a run on a site (G16)

const NO_PROJECT = 'No project is open: open a project folder first (Open project folder…).';
const caseParam = { type: 'string', description: 'The case file, project-relative (cases/cavity.toml).' } as const;
const siteParam = { type: 'string', description: 'A site of the user’s sites file (rcfd site list), confirmed once.' } as const;

const SITE_PARAMS = {
  case: caseParam,
  site: siteParam,
  priority: { type: 'integer', optional: true, description: 'Higher runs first among the queued runs (default 0).' },
  threads: { type: 'integer', optional: true, minimum: 1, maximum: 256, description: 'Solver threads of the run (default 1).' },
  mem_mb: { type: 'integer', optional: true, minimum: 1, description: 'Memory the site’s runner books for the run, MB (default: its estimate).' },
  set: { type: 'string-array', optional: true, description: 'Settings for this run only, each path=value (rcfd run --set), in order.' },
} as const;

const RUN_ON_SITE: RunActionSpec<typeof SITE_PARAMS> = {
  title: 'Run the case on a site',
  description:
    'Start a run of a project case on a remote site of the user’s sites file: the run is queued here, its inputs are sent to the site, the site runs it, and its folder here is kept up to date from the site; outputs and fields are copied back when it ends.',
  group: 'run',
  params: SITE_PARAMS,
  engine: { kind: 'runner', command: 'start' },
  mcpNote:
    'start_run takes site only for a site with assistant = true in the sites file (ADR 0739 Q6); the Studio starts the run with rcfd-runner start --site, a direct action of the user, after the dry run of start_run gives the store plan’s settings.',
  cli: (p, project) => [
    'rcfd-runner',
    'start',
    p.case,
    ...(p.priority !== undefined ? ['--priority', String(p.priority)] : []),
    ...(p.threads !== undefined ? ['--threads', String(p.threads)] : []),
    ...(p.mem_mb !== undefined ? ['--mem-mb', String(p.mem_mb)] : []),
    ...(p.set ?? []).flatMap((s) => ['--set', s]),
    '--site',
    p.site,
    '--project',
    project,
  ],
  cliNote:
    'The Studio adds the store plan’s settings (from start_run’s dry run) as --set entries before the run’s own, so the line it shows after the run is the one it ran. rcfd-runner start writes no line in the project journal.',
  unavailable: (ctx: RunActionContext, p) => {
    if (ctx.project === false) return NO_PROJECT;
    if (p.case !== undefined && ctx.cases !== undefined && !ctx.cases.includes(p.case)) {
      return `${p.case} is not a case file of this project (cases/): it may have been moved or deleted. Choose one of the cases the project lists.`;
    }
    if (p.site !== undefined && (!isSiteName(p.site) || p.site === LOCAL)) return `“${String(p.site)}” is not a site name of the sites file (letters, digits, _ and -).`;
    const bad = (p.set ?? []).find((s) => !s.includes('='));
    return bad !== undefined ? `“${bad}” is not a setting for the run: write each one as path=value (solver.max_iterations=500).` : null;
  },
  expect: 'new_run',
};

/** The site actions (kept out of RUN_ACTIONS, as the first path's are: the palette and the F-807 lists stay as they are). */
export const SITE_RUN_ACTIONS: readonly RegisteredRunAction[] = [
  Object.freeze({ ...(RUN_ON_SITE as unknown as RunActionSpec<ParamsSpec>), id: 'site.run_case' as unknown as RunActionId, effect: 'change' as const }),
];

export const SITE_RUN_CASE = 'site.run_case';

export function getSiteRunAction(id: string): RegisteredRunAction | undefined {
  return SITE_RUN_ACTIONS.find((a) => (a.id as string) === id);
}
