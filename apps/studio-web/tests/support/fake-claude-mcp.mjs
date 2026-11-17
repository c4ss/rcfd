// A stand-in for the `claude` CLI's MCP commands (ADR 0801 Addendum 23), for the tests of "Use with Claude Code": it
// records each call's argv and working folder, one JSON line each, to the file FAKE_CLAUDE_LOG names, and keeps the
// registrations, by working folder (local scope is keyed by the folder), in the JSON file FAKE_CLAUDE_STATE names. Its
// output copies the real CLI's (checked on Claude Code 2.1.326):
//
//   claude mcp get <name>                          exit 1 "No MCP server named ..." or exit 0 with Scope/Command/Args
//   claude mcp add <name> --scope local -- <cmd> [args...]    exit 1 when the name is there already
//   claude mcp remove <name> --scope local         exit 1 when there is none
//
// FAKE_CLAUDE_GET_FAILS=1 makes `get` answer something else (exit 3), for the "did not answer" path.
// The Studio runs a .mjs claude through its own Electron as Node (src/main/claudecode/terminal.ts).
import { appendFileSync, existsSync, readFileSync, writeFileSync } from 'node:fs';

const argv = process.argv.slice(2);
const cwd = process.cwd();
if (process.env.FAKE_CLAUDE_LOG) appendFileSync(process.env.FAKE_CLAUDE_LOG, `${JSON.stringify({ argv, cwd })}\n`);

const stateFile = process.env.FAKE_CLAUDE_STATE ?? '';
const load = () => (stateFile && existsSync(stateFile) ? JSON.parse(readFileSync(stateFile, 'utf8')) : {});
const save = (s) => stateFile && writeFileSync(stateFile, JSON.stringify(s, null, 2));
const key = cwd.replace(/\\/g, '/').toLowerCase();

const [group, cmd, name, ...rest] = argv;
if (group !== 'mcp') {
  process.stderr.write(`fake claude: only mcp commands, not ${argv.join(' ')}\n`);
  process.exit(2);
}
const state = load();
const here = state[key] ?? {};
if (cmd === 'get') {
  if (process.env.FAKE_CLAUDE_GET_FAILS === '1') {
    process.stderr.write('something unexpected\n');
    process.exit(3);
  }
  const s = here[name];
  if (!s) {
    process.stdout.write(`No MCP server named "${name}". Configured servers: (none)\n`);
    process.exit(1);
  }
  process.stdout.write(
    `${name}:\n  Scope: Local config (private to you in this project)\n  Status: ✔ Connected\n  Type: stdio\n  Command: ${s.command}\n  Args: ${s.args.join(' ')}\n  Environment:\n\nTo remove this server, run: claude mcp remove ${name} -s local\n`,
  );
  process.exit(0);
}
if (cmd === 'add') {
  const dash = rest.indexOf('--');
  const scope = rest[rest.indexOf('--scope') + 1];
  if (dash < 0 || scope !== 'local') {
    process.stderr.write('fake claude: add needs --scope local and --\n');
    process.exit(2);
  }
  if (here[name]) {
    process.stdout.write(`MCP server ${name} already exists in local config\n`);
    process.exit(1);
  }
  const [command, ...args] = rest.slice(dash + 1);
  state[key] = { ...here, [name]: { command, args } };
  save(state);
  process.stdout.write(`Added stdio MCP server ${name} with command: ${command} ${args.join(' ')} to local config\n`);
  process.exit(0);
}
if (cmd === 'remove') {
  if (!here[name]) {
    process.stdout.write(`No MCP server named "${name}" in local scope\n`);
    process.exit(1);
  }
  delete here[name];
  state[key] = here;
  save(state);
  process.stdout.write(`Removed MCP server ${name} from local config\n`);
  process.exit(0);
}
process.stderr.write(`fake claude: unknown mcp command ${cmd}\n`);
process.exit(2);
