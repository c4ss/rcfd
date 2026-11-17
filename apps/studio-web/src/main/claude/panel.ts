// The Claude Code panel of the main process (F-814): the open project's sessions, the selected session's transcript, and
// the Studio's own turns. The renderer reaches it only through the Claude port (bridge.ts) and gets the whole state after
// every change. Every process and file operation is here, never in the renderer (G10).
//
// The rules of G11 and docs/ui-restart-plan.md section 8:
// - Listing: every DISCOVER_MS while a project is open (discover.ts). OPEN sessions are mirrored read-only; UNKNOWN ones
//   (the listing failed, or claude is not configured) are not written to.
// - Writing: `send` resumes a CLOSED session (`--resume`) or starts a new one with an id chosen here (`--session-id`).
//   Right before the spawn the session's state is listed again; a session open in a live process is refused. The
//   one-writer lock (`writer`) is taken before the first check and held until the process has ended; a second message
//   while it is held is refused with `busy`.
// - Approvals: the allow-list (allow.ts): the project's `.rcfd/claude-allow.json` when it is valid, else the Studio's
//   first list of read-only project tools; a file with one bad rule is refused as a whole and no chat starts. The panel
//   shows a denied call and the rule that would allow it. "Allow this from now on" (the maintainer, 2026-11-17) adds
//   exactly that rule to the file once the user has confirmed it (`allowRule`): only a rule suggested for a denied call
//   in the feed, never over a file changed since it was read, validated as the file's own rules are, and the folder's
//   confirmation of its control files follows the Studio's own write; `undoAllow` takes the last one out again.
// - The folder's Claude Code control files must be confirmed first (trust.ts); the record is in the user data.
// - MCP: an app-owned config in the user data, passed with --strict-mcp-config, naming one server, the engine's project
//   server `rcfd-mcp --project <folder> --root <folder>` (ADR 0134 W48-10 (a)); with no rcfd-mcp configured, no server.
//
// No Electron import: unit-tested in Node with the fake claude of tests/support.
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, realpath, rename, writeFile } from 'node:fs/promises';
import path from 'node:path';
import {
  DISCOVER_MS,
  FLOOR_CAVEAT,
  isUuid,
  MAX_MESSAGE_CHARS,
  MIRROR_MS,
  PINNED_CLI_VERSION,
  SHOWN_CHARS,
  TAIL,
  type AllowView,
  type ClaudeError,
  type ClaudeState,
  type ClaudeView,
  type ControlFileView,
  type FeedItem,
  type FolderTrust,
  type SessionRow,
  type TurnView,
} from '../../shared/claude.js';
import { ALLOW_FILE, type AllowFileResult, allowFileHash, allowFileText, AllowList, flagValue, parseAllowFile, PINNED_SERVER, readAllowText, suggest, withRule, writeAllowFile } from './allow.js';
import type { ClaudeConfig } from './config.js';
import { classify, isAlive, projectDirName, type Discovery, type HeadCache } from './discover.js';
import { chatArgs, listAgents, startTurn, type Listing, type Turn, type TurnEvent } from './driver.js';
import { readNew, tail, type TranscriptEntry } from './transcript.js';
import { checkFolder, isAllowFile, TrustStore } from './trust.js';

/** A request the panel refuses; bridge.ts turns it into the reply's error. */
export class ClaudeRefusal extends Error {
  constructor(
    readonly code: ClaudeError['code'],
    message: string,
  ) {
    super(message);
  }
}

export interface PanelDeps {
  readonly config: ClaudeConfig;
  /** The engine's project server for the app-owned MCP config; null when the Studio was not told where it is. */
  readonly rcfdMcp: string | null;
  listAgents?(cwd: string): Promise<Listing>;
  alive?(pid: number): boolean;
  now?(): string;
  discoverMs?: number;
  mirrorMs?: number;
}

const clip = (s: string, n = SHOWN_CHARS) => (s.length > n ? `${s.slice(0, n)}…` : s);

interface Mirror {
  session: string;
  file: string;
  offset: number | null;
}

export class ClaudePanel {
  private folder: string | null = null;
  private forms: string[] = [];
  private discovery: Discovery = { sessions: [], warnings: [] };
  private listedAt: string | null = null;
  private selected: string | null = null;
  private feed: FeedItem[] = [];
  private feedError: string | null = null;
  private turn: TurnView | null = null;
  /** F-837: the cost the CLI reported for each turn this Studio sent, summed by session (USD). */
  private readonly costs = new Map<string, number>();
  private running: Turn | null = null;
  private writer: { session: string; pid: number | null } | null = null;
  private allow: AllowView = { source: 'initial', file: ALLOW_FILE, rules: AllowList.initial().values, error: null, hash: null, undo: null };
  /** The rule this Studio last added: the folder, the rule, the file's text before (null: none) and the hash it left. */
  private lastAllow: { folder: string; rule: string; before: string | null; after: string } | null = null;
  private allowList: AllowList | null = AllowList.initial();
  private trust: FolderTrust = { state: 'confirmed', files: 0 };
  private mcp: ClaudeView['mcp'] = { file: null, server: null, note: null };
  private versionWarning: string | null = null;
  private mirror: Mirror | null = null;
  private mirrorPaused = false;
  /** The running turn's tool inputs (for the rule a denied call would need) and whether its text item is open. */
  private inputs = new Map<string, unknown>();
  private textOpen = false;
  private cancelled = false;
  /** Processes this Studio started: a listing that still shows one after it ended does not make its session read-only. */
  private readonly ownPids = new Set<number>();
  private provisionalFrom: number | null = null;
  private keyN = 0;
  private generation = 0;
  private discovering = false;
  private mirroring = false;
  private timers: ReturnType<typeof setInterval>[] = [];
  private readonly heads: HeadCache = new Map();
  private readonly listeners = new Set<(s: ClaudeState) => void>();
  private emitTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly store: TrustStore;
  private readonly now: () => string;
  private readonly list: (cwd: string) => Promise<Listing>;
  private readonly alive: (pid: number) => boolean;

  constructor(private readonly deps: PanelDeps) {
    this.store = new TrustStore(path.join(deps.config.stateDir, 'claude-trust.json'));
    this.now = deps.now ?? (() => new Date().toISOString());
    this.list = deps.listAgents ?? ((cwd) => listAgents(deps.config, cwd));
    this.alive = deps.alive ?? isAlive;
  }

  // ---- state

  get state(): ClaudeState {
    return { program: this.deps.config.program, home: this.deps.config.home, project: this.view() };
  }

  private view(): ClaudeView | null {
    if (this.folder === null) return null;
    return {
      folder: this.folder,
      transcripts: this.transcriptDir(),
      sessions: this.rows(),
      warnings: this.discovery.warnings,
      listedAt: this.listedAt,
      selected: this.selected,
      feed: this.feed,
      feedError: this.feedError,
      turn: this.turn,
      writer: this.writer,
      allow: { ...this.allow, undo: this.lastAllow?.folder === this.folder ? this.lastAllow.rule : null },
      trust: this.trust,
      mcp: this.mcp,
      versionWarning: this.versionWarning,
    };
  }

  private transcriptDir(): string {
    return this.deps.config.home ? path.join(this.deps.config.home, 'projects', projectDirName(this.folder ?? '')) : '';
  }

  private rows(): SessionRow[] {
    const rows: SessionRow[] = this.discovery.sessions.map((s) => {
      const mine = this.writer?.session === s.session;
      const reason = this.discovery.warnings.find((w) => w.startsWith('claude agents')) ?? 'the session listing has not run yet';
      const ended = s.state.kind === 'open' && this.ownPids.has(s.state.pid) && !this.alive(s.state.pid);
      const open = mine ? 'studio' : ended ? 'closed' : s.state.kind;
      const pid = mine ? this.writer!.pid : open === 'open' && s.state.kind === 'open' ? s.state.pid : null;
      return {
        id: s.session,
        name: s.name,
        title: s.title,
        modified: s.modified === null ? null : new Date(s.modified).toISOString(),
        open,
        pid,
        readOnly:
          open === 'studio'
            ? 'This Studio is writing to this session now (one writer per session); wait for the reply or stop it.'
            : open === 'open'
              ? `This session is open in process ${pid}; it is mirrored read-only until that process ends.`
              : open === 'unknown'
                ? `Whether another process has this session open cannot be established (${reason}); it is not written to.`
                : null,
      };
    });
    // a session this Studio has just started may not be listed yet
    if (this.writer && !rows.some((r) => r.id === this.writer!.session)) {
      rows.unshift({
        id: this.writer.session,
        name: null,
        title: null,
        modified: null,
        open: 'studio',
        pid: this.writer.pid,
        readOnly: 'This Studio is writing to this session now (one writer per session); wait for the reply or stop it.',
      });
    }
    return rows;
  }

  onState(listener: (s: ClaudeState) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Coalesces the pushes of a streaming turn (at most one every 30 ms). */
  private emit(): void {
    if (this.emitTimer) return;
    this.emitTimer = setTimeout(() => {
      this.emitTimer = null;
      const s = this.state;
      for (const l of this.listeners) {
        try {
          l(s);
        } catch {
          // a listener's failure is its own
        }
      }
    }, 30);
  }

  // ---- the open project

  /** Follows the project home: a new folder, or none. */
  async setProject(folder: string | null): Promise<void> {
    if (folder === this.folder) return;
    this.stop();
    this.generation++;
    this.folder = folder;
    this.discovery = { sessions: [], warnings: [] };
    this.listedAt = null;
    this.selected = null;
    this.feed = [];
    this.feedError = null;
    this.turn = null;
    this.mirror = null;
    this.versionWarning = null;
    this.heads.clear();
    this.emit();
    if (folder === null) return;
    const gen = this.generation;
    let real = folder;
    try {
      real = await realpath(folder);
    } catch {
      // the folder as opened
    }
    if (gen !== this.generation) return;
    this.forms = real === folder ? [folder] : [folder, real];
    await Promise.all([this.checkFolder(), this.readAllow(), this.discover()]);
    if (gen !== this.generation) return;
    this.timers.push(setInterval(() => void this.discover(), this.deps.discoverMs ?? DISCOVER_MS));
    this.timers.push(setInterval(() => void this.mirrorTick(), this.deps.mirrorMs ?? MIRROR_MS));
  }

  private stop(): void {
    for (const t of this.timers) clearInterval(t);
    this.timers = [];
    this.running?.cancel();
  }

  /** Stops the timers and the running turn (the Studio quits). */
  async shutdown(): Promise<void> {
    this.stop();
    await this.running?.done;
  }

  private async discover(): Promise<void> {
    if (this.discovering || this.folder === null || this.deps.config.home === null) {
      if (this.deps.config.home === null && this.folder !== null) {
        this.discovery = { sessions: [], warnings: ["Claude Code's home folder is not known (CLAUDE_CONFIG_DIR, USERPROFILE or HOME unset), so its sessions cannot be listed."] };
        this.emit();
      }
      return;
    }
    this.discovering = true;
    const gen = this.generation;
    try {
      const listing = await this.list(this.folder);
      const d = await classify(this.forms, this.deps.config.home, listing, this.alive, this.heads);
      if (gen !== this.generation) return;
      this.discovery = d;
      this.listedAt = this.now();
      this.emit();
    } finally {
      this.discovering = false;
    }
  }

  private async checkFolder(): Promise<FolderTrust> {
    if (this.folder === null) return this.trust;
    const gen = this.generation;
    const { trust } = await checkFolder(this.folder, this.store);
    if (gen === this.generation) {
      this.trust = trust;
      this.emit();
    }
    return trust;
  }

  private async readAllow(): Promise<void> {
    if (this.folder === null) return;
    const gen = this.generation;
    let text: string | null;
    let r: AllowFileResult;
    try {
      text = await readAllowText(this.folder);
      r = text === null ? { kind: 'absent' } : parseAllowFile(text);
    } catch (e) {
      text = null;
      r = { kind: 'refused', reason: `${ALLOW_FILE}: ${(e as Error).message}` };
    }
    if (gen !== this.generation) return;
    const hash = allowFileHash(text);
    if (r.kind === 'list') {
      this.allowList = r.list;
      this.allow = { source: 'file', file: ALLOW_FILE, rules: r.list.values, error: null, hash, undo: null };
    } else if (r.kind === 'absent') {
      this.allowList = AllowList.initial();
      this.allow = { source: 'initial', file: ALLOW_FILE, rules: this.allowList.values, error: null, hash: null, undo: null };
    } else {
      this.allowList = null;
      this.allow = { source: 'file', file: ALLOW_FILE, rules: [], error: r.reason, hash, undo: null };
    }
    this.emit();
  }

  // ---- "Allow this from now on" (F-814, the maintainer's approval of 2026-11-17)

  /**
   * Adds `rule` to the project's allow-list file. Refused unless it is the rule suggested for a denied call the feed
   * shows, the folder's control files are confirmed, and the file still has the hash `base` that was shown (null: no
   * file); the new list passes the checks of a file read from the folder and of every spawn (allow.ts `withRule`).
   */
  async allowRule(rule: string, base: string | null): Promise<void> {
    if (this.folder === null) throw new ClaudeRefusal('no_project', 'No project is open.');
    const folder = this.folder;
    if (!this.feed.some((i) => i.kind === 'denied' && i.suggestion === rule)) {
      throw new ClaudeRefusal('bad_request', 'Only the rule this panel suggested for a denied call it shows can be added.');
    }
    const trust = await this.checkFolder();
    if (trust.state !== 'confirmed') throw new ClaudeRefusal('not_confirmed', 'Confirm the folder’s Claude Code files first; nothing was written.');
    const text = await readAllowText(folder);
    if (allowFileHash(text) !== base) {
      await this.readAllow();
      throw new ClaudeRefusal('stale', `${ALLOW_FILE} was changed by another writer since it was read: nothing was written. It has been read again; allow the call again to add the rule to it.`);
    }
    const next = withRule(text, rule);
    if ('refused' in next) throw new ClaudeRefusal('refused', `Nothing was written: ${next.refused}.`);
    await this.writeOwn(folder, next.text, base, `${ALLOW_FILE} was changed by another writer while the rule was being added: nothing was written.`);
    this.lastAllow = { folder, rule, before: text, after: allowFileHash(next.text)! };
    await this.readAllow();
    this.emit();
  }

  /**
   * Takes the rule this Studio last added out again: the file goes back to the text it had (no file before: the
   * Studio's first list, written as a file, since the Studio removes no file of a project). A file changed since by
   * another writer is refused (`stale`) unless `force`.
   */
  async undoAllow(force: boolean): Promise<void> {
    if (this.folder === null) throw new ClaudeRefusal('no_project', 'No project is open.');
    const folder = this.folder;
    const last = this.lastAllow;
    if (!last || last.folder !== folder) throw new ClaudeRefusal('bad_request', 'Nothing to undo: this Studio has added no rule to this project’s allow-list.');
    const now = allowFileHash(await readAllowText(folder));
    if (now !== last.after && !force) {
      await this.readAllow();
      throw new ClaudeRefusal('stale', `${ALLOW_FILE} was changed by another writer after ${last.rule} was added: nothing was undone. Undo anyway puts back the list as it was before the rule was added.`);
    }
    const before = last.before ?? allowFileText(AllowList.initial().values);
    await this.writeOwn(folder, before, now, `${ALLOW_FILE} was changed by another writer while it was being put back: nothing was undone.`);
    this.lastAllow = null;
    await this.readAllow();
    this.emit();
  }

  /**
   * The Studio's own write of the allow-list file, and the confirmation of the folder's control files that follows it:
   * when the folder was confirmed and the write is the only change of its control files, the new scan is recorded as
   * confirmed (the user confirmed this exact change); any other change still asks.
   */
  private async writeOwn(folder: string, text: string, base: string | null, conflict: string): Promise<void> {
    const before = await checkFolder(folder, this.store);
    if (!(await writeAllowFile(folder, text, base))) {
      await this.readAllow();
      throw new ClaudeRefusal('stale', conflict);
    }
    const after = await checkFolder(folder, this.store);
    const others = (files: readonly ControlFileView[]) => JSON.stringify(files.filter((f) => !isAllowFile(f.path)));
    const written = after.files.find((f) => isAllowFile(f.path));
    if (
      before.trust.state === 'confirmed' &&
      after.trust.state === 'pending' &&
      others(before.files) === others(after.files) &&
      written?.kind === 'file' &&
      written.hash === `sha256:${allowFileHash(text)}`
    ) {
      await this.store.confirm(folder, after.files);
    }
    await this.checkFolder();
  }

  /** The app-owned MCP config of this project, in the user data (never in the project), written before each turn. */
  private async writeMcpConfig(): Promise<string> {
    const folder = this.folder!;
    const servers = this.deps.rcfdMcp
      ? { [PINNED_SERVER]: { type: 'stdio', command: this.deps.rcfdMcp, args: ['--project', folder, '--root', folder] } }
      : {};
    const dir = path.join(this.deps.config.stateDir, 'mcp');
    const file = path.join(dir, `${createHash('sha256').update(folder).digest('hex').slice(0, 24)}.json`);
    await mkdir(dir, { recursive: true });
    const tmp = `${file}.${process.pid}.tmp`;
    await writeFile(tmp, JSON.stringify({ mcpServers: servers }, null, 2) + '\n');
    await rename(tmp, file);
    this.mcp = {
      file,
      server: this.deps.rcfdMcp ? `${this.deps.rcfdMcp} --project <folder> --root <folder>` : null,
      note: this.deps.rcfdMcp
        ? null
        : "No project tools: the Studio was not told where the engine's project server is (RCFD_STUDIO_MCP), so Claude Code gets no MCP server.",
    };
    return file;
  }

  // ---- the transcript of the selected session

  private item(kind: FeedItem['kind'], text: string, extra: Partial<FeedItem> = {}): FeedItem {
    return { key: `i${++this.keyN}`, kind, text, tools: [], at: null, provisional: false, suggestion: null, ...extra };
  }

  private fromEntry(e: TranscriptEntry): FeedItem {
    if (e.kind === 'unrecognised') return this.item('unrecognised', `A transcript line of a shape this Studio does not read${e.type ? ` (${e.type})` : ''}.`);
    if (e.role === 'tool_result') return this.item('tool_result', clip(e.text), { at: e.at });
    if (e.role === 'assistant' && e.text.trim() === '' && e.tools.length > 0) return this.item('tool', '', { tools: e.tools, at: e.at });
    return this.item(e.role, e.text, { tools: e.tools, at: e.at });
  }

  private transcriptOf(session: string): string | null {
    const known = this.discovery.sessions.find((s) => s.session === session)?.transcript;
    if (known) return known;
    return this.deps.config.home ? path.join(this.transcriptDir(), `${session}.jsonl`) : null;
  }

  private async mirrorTick(): Promise<void> {
    const m = this.mirror;
    if (!m || this.mirrorPaused || this.mirroring) return;
    this.mirroring = true;
    const gen = this.generation;
    try {
      if (m.offset === null) {
        const r = await tail(m.file, TAIL);
        if (gen !== this.generation || this.mirror !== m) return;
        m.offset = r.offset;
        // after a turn of a new session, the transcript's first read replaces what the stream showed
        const replace = this.provisionalFrom !== null && !this.running && r.entries.length > 0;
        if (replace) this.provisionalFrom = null;
        this.feed = [...r.entries.map((e) => this.fromEntry(e)), ...(replace ? this.keptDenials() : this.feed.filter((i) => i.provisional))];
        this.feedError = null;
        this.emit();
        return;
      }
      const r = await readNew(m.file, m.offset);
      if (gen !== this.generation || this.mirror !== m) return;
      m.offset = r.offset;
      if (r.entries.length > 0 || r.reset) {
        // the CLI has written the turn's lines: they replace what the stream showed
        let kept: FeedItem[] = [];
        if (this.provisionalFrom !== null) {
          kept = this.keptDenials();
          this.feed = this.feed.filter((i) => !i.provisional);
          this.provisionalFrom = null;
        }
        const items = r.entries.map((e) => this.fromEntry(e));
        this.feed = r.reset ? [...items, ...kept] : [...this.feed, ...items, ...kept];
        this.feedError = null;
        this.emit();
      }
    } catch (e) {
      if (gen !== this.generation || this.mirror !== m) return;
      if ((e as NodeJS.ErrnoException).code === 'ENOENT') return; // a new session's transcript appears later
      const msg = `The transcript ${m.file} cannot be read: ${(e as Error).message}`;
      if (this.feedError !== msg) {
        this.feedError = msg;
        this.emit();
      }
    } finally {
      this.mirroring = false;
    }
  }

  /**
   * The denied calls of the turn that streamed, kept when the transcript's lines replace the stream: the transcript does
   * not carry the rule a denial would need, and "Allow this from now on" stays reachable after the reply (F-814).
   */
  private keptDenials(): FeedItem[] {
    return this.feed.filter((i) => i.provisional && i.kind === 'denied').map((i) => ({ ...i, provisional: false }));
  }

  /** Mirrors a session (or none: a new chat). Refused while a turn runs. */
  select(session: string | null): void {
    if (this.folder === null) throw new ClaudeRefusal('no_project', 'No project is open.');
    if (this.running) throw new ClaudeRefusal('busy', 'A message is being answered; wait for the reply or stop it, then choose another session.');
    if (session !== null && !this.rows().some((r) => r.id === session)) {
      throw new ClaudeRefusal('bad_request', 'That session is not one of this project’s sessions.');
    }
    if (session === this.selected) return;
    this.selected = session;
    this.feed = [];
    this.feedError = null;
    this.provisionalFrom = null;
    const file = session === null ? null : this.transcriptOf(session);
    this.mirror = session !== null && file !== null ? { session, file, offset: null } : null;
    this.emit();
    void this.mirrorTick();
  }

  // ---- writing

  /** Sends a message. Resolves with the session written to once the process has started. */
  async send(session: string | null, text: string): Promise<{ session: string }> {
    if (this.folder === null) throw new ClaudeRefusal('no_project', 'No project is open.');
    const cfg = this.deps.config;
    if (!cfg.program.configured) throw new ClaudeRefusal('not_configured', cfg.program.reason);
    if (text.trim() === '') throw new ClaudeRefusal('bad_request', 'The message is empty.');
    if (text.length > MAX_MESSAGE_CHARS) throw new ClaudeRefusal('bad_request', `The message is longer than ${MAX_MESSAGE_CHARS} characters.`);
    if (session !== null && !isUuid(session)) throw new ClaudeRefusal('bad_request', 'Not a session id (a UUID is expected).');
    // the one-writer lock: taken before any check that waits, released when the process has ended or a check refused
    if (this.writer) {
      throw new ClaudeRefusal(
        'busy',
        this.writer.session === session
          ? 'This Studio is already writing to this session (one writer per session); wait for the reply or stop it.'
          : 'This Studio is answering a message in another session; wait for the reply or stop it.',
      );
    }
    const isNew = session === null;
    const id = session ?? randomUUID();
    const folder = this.folder;
    const gen = this.generation;
    this.writer = { session: id, pid: null };
    this.turn = this.turnView(id, isNew, 'starting', null, []);
    this.emit();
    const refuse = (code: ClaudeError['code'], message: string): never => {
      if (gen === this.generation) {
        this.writer = null;
        this.turn = { ...this.turn!, status: 'refused', message };
        this.emit();
      }
      throw new ClaudeRefusal(code, message);
    };
    if (!isNew) {
      const row = this.rows().find((r) => r.id === id);
      if (!row) refuse('bad_request', 'That session is not one of this project’s sessions.');
      if (row!.open === 'open') refuse('read_only', row!.readOnly!);
      if (row!.open === 'unknown') refuse('read_only', row!.readOnly!);
    }
    const trust = await this.checkFolder();
    if (trust.state === 'pending') {
      refuse('not_confirmed', `This folder holds Claude Code control files that are not confirmed (${trust.changed.join(', ')}); review and confirm them first.`);
    }
    if (trust.state === 'error') refuse('not_confirmed', trust.message);
    await this.readAllow();
    if (!this.allowList) refuse('refused', this.allow.error ?? 'The allow-list file is refused.');
    if (!isNew) {
      // right before the spawn: is the session open in another process? (any folder: the session is the unit)
      const listing = await this.list(folder);
      if (!listing.ok) refuse('read_only', `Whether another process has this session open cannot be established (claude agents --json: ${listing.reason}); not writing to it.`);
      const live = (listing as { agents: { session: string; pid: number | null }[] }).agents.find((a) => a.session === id && a.pid !== null && this.alive(a.pid));
      if (live) refuse('read_only', `This session is open in process ${live.pid}; it can only be mirrored read-only until that process ends.`);
    }
    if (gen !== this.generation) throw new ClaudeRefusal('no_project', 'The project was closed.');
    let argv: string[];
    let turn: Turn;
    try {
      const mcp = await this.writeMcpConfig();
      argv = chatArgs(isNew ? '--session-id' : '--resume', id, this.allowList!, mcp);
      turn = await startTurn(cfg, folder, argv, text, (e) => this.onTurnEvent(gen, id, e));
    } catch (e) {
      return refuse('refused', (e as Error).message);
    }
    this.running = turn;
    this.cancelled = false;
    if (turn.pid !== null) this.ownPids.add(turn.pid);
    this.writer = { session: id, pid: turn.pid };
    this.turn = this.turnView(id, isNew, 'running', null, turn.argv, turn.pid);
    // the feed shows the session being written to
    if (this.selected !== id) {
      this.selected = id;
      this.feed = [];
      this.feedError = null;
      const file = this.transcriptOf(id);
      this.mirror = file ? { session: id, file, offset: null } : null;
    }
    this.mirrorPaused = true;
    this.provisionalFrom = this.feed.length;
    this.feed = [...this.feed, this.item('user', text, { provisional: true, at: this.now() })];
    this.emit();
    void turn.done.then(() => {
      if (gen !== this.generation) return;
      this.running = null;
      this.writer = null;
      this.mirrorPaused = false;
      this.emit();
      void this.discover();
      void this.mirrorTick();
    });
    return { session: id };
  }

  private turnView(session: string, isNew: boolean, status: TurnView['status'], message: string | null, argv: readonly string[], pid: number | null = null): TurnView {
    return { session, isNew, status, message, pid, argv, started: this.now(), exitCode: null, stderrTail: '', mode: null, version: null, model: null, costUsd: null, sessionCostUsd: this.costs.get(session) ?? null };
  }

  private onTurnEvent(gen: number, session: string, e: TurnEvent): void {
    if (gen !== this.generation || !this.turn || this.turn.session !== session) return;
    const t = this.turn;
    const shown = this.selected === session;
    const push = (i: FeedItem) => {
      if (shown) this.feed = [...this.feed, i];
    };
    switch (e.kind) {
      case 'init':
        this.turn = { ...t, mode: e.mode, version: e.version, model: e.model };
        this.versionWarning =
          e.version && e.version !== PINNED_CLI_VERSION
            ? `Claude Code ${e.version} answered; this Studio reads the stream and transcript shapes of ${PINNED_CLI_VERSION}, so some lines may show as unrecognised.`
            : null;
        break;
      case 'text': {
        const last = this.feed[this.feed.length - 1];
        if (shown && this.textOpen && last?.kind === 'assistant' && last.provisional) {
          this.feed = [...this.feed.slice(0, -1), { ...last, text: last.text + e.text }];
        } else {
          push(this.item('assistant', e.text, { provisional: true }));
          this.textOpen = true;
        }
        break;
      }
      case 'tool':
        this.inputs.set(e.id, e.input);
        this.textOpen = false;
        push(this.item('tool', clip(JSON.stringify(e.input ?? null)), { tools: [e.name], provisional: true }));
        break;
      case 'denied': {
        this.textOpen = false;
        const rule = suggest(e.tool, this.inputs.get(e.toolUseId), this.folder ?? '');
        push(this.item('denied', `${e.tool}: ${e.message}`, { tools: [e.tool], provisional: true, suggestion: rule ? flagValue(rule) : null }));
        break;
      }
      case 'result':
        this.textOpen = false;
        if (e.costUsd !== null) this.costs.set(t.session, (this.costs.get(t.session) ?? 0) + e.costUsd);
        this.turn = { ...t, costUsd: e.costUsd, sessionCostUsd: this.costs.get(t.session) ?? null, status: e.isError ? 'failed' : t.status, message: e.isError ? (e.text ?? `the turn ended with ${e.subtype}`) : t.message };
        break;
      case 'aborted':
        this.turn = { ...t, status: 'aborted', message: e.reason };
        push(this.item('note', `Stopped: ${e.reason}`, { provisional: true }));
        break;
      case 'exit': {
        const status = t.status === 'running' ? (this.cancelled ? 'cancelled' : e.code === 0 ? 'finished' : 'failed') : t.status;
        this.turn = {
          ...t,
          status,
          exitCode: e.code,
          stderrTail: e.stderrTail,
          message: status === 'failed' && t.message === null ? `claude ended with exit code ${e.code}${e.stderrTail.trim() ? `: ${clip(e.stderrTail.trim(), 400)}` : ''}` : status === 'cancelled' ? 'Stopped by you.' : t.message,
        };
        if (status === 'cancelled') push(this.item('note', 'Stopped by you. The transcript keeps what was written before.', { provisional: true }));
        this.inputs.clear();
        this.textOpen = false;
        break;
      }
      case 'unrecognised':
        break;
    }
    this.emit();
  }

  /** Stops the running turn. */
  cancel(): void {
    if (!this.running || !this.turn) throw new ClaudeRefusal('bad_request', 'No message is being answered.');
    this.cancelled = true;
    this.running.cancel();
  }

  /** Records the folder's control files as confirmed, if they are still the ones shown under `token`. */
  async confirm(token: string): Promise<void> {
    if (this.folder === null) throw new ClaudeRefusal('no_project', 'No project is open.');
    const { trust, files } = await checkFolder(this.folder, this.store);
    if (trust.state === 'confirmed') {
      this.trust = trust;
      this.emit();
      return;
    }
    if (trust.state !== 'pending' || trust.token !== token) {
      this.trust = trust;
      this.emit();
      throw new ClaudeRefusal('stale', 'The folder’s control files changed after they were shown; review them again.');
    }
    await this.store.confirm(this.folder, files);
    await this.checkFolder();
  }

  /** Lists the sessions, reads the allow-list and checks the folder again now. */
  async refresh(): Promise<void> {
    if (this.folder === null) throw new ClaudeRefusal('no_project', 'No project is open.');
    await Promise.all([this.discover(), this.readAllow(), this.checkFolder()]);
  }

  /** For the panel's words: what the allow-list does not cover. */
  static readonly caveat = FLOOR_CAVEAT;
}
