# Harness history

A harness lists its past sessions in the Sessions sidebar and the Home activity map through
its manifest's `history` field. Axome only reads harness stores; it never writes them.

```jsonc
"history": { "builtin": "omp" }                                   // a scanner shipped with Axome
"history": { "command": "./history", "args": [], "env": {} }      // a process your extension ships
```

`builtin` and `command` are exclusive. Builtins are `omp`, `claude`, `codex` and `antigravity`.

## History commands

`command` resolves like other contribution commands: `./name` from the extension folder, a bare
name from the launch `PATH`.

### When it runs

- **Listing:** when a folder's history is shown or refreshed, when a session ends, when the Home
  view needs every folder, and when a path the command asked Axome to watch changes.
- **Binding:** after Axome launches a new terminal session, it lists with `since` set to the
  launch time, every second for 30 seconds and every 5 seconds after that, until a new session
  appears to claim.
- **Resume:** before resuming, Axome lists the folder to confirm the session still exists.

Each run is one process per preset. A command that hasn't exited after 30 seconds is killed with
its whole process group, and the listing fails.

### Environment

- The environment is cleared, then set to the preset's launch environment (the harness CLI's
  `PATH`, `HOME`, preset `env`) plus `history.env`. Read your store location from the same
  variables your CLI reads.
- The working directory is the folder being listed, else `HOME`.

### Request (stdin)

One JSON line:

```json
{"api": 1, "folder": "/abs/project", "args": ["--profile", "work"], "since": "2026-11-01T19:46:02.870Z"}
```

| Field | Meaning |
|---|---|
| `folder` | Folder to list, or `null` for every folder the store knows (Home view). |
| `args` | Harness `extra_args` then preset args, as the CLI receives them. Honor flags that move the store. |
| `since` | Optional hint, floored to whole seconds: you may omit sessions created before it. |

### Response (stdout)

One JSON object per line, in any order. Blank lines are ignored.

A **session**:

```json
{"id": "abc123", "cwd": "/abs/project", "title": "Fix the parser", "last_message": {"role": "agent", "text": "The parser now handles escaped delimiters."}, "message_count": 12, "created_at": "2026-11-01T19:46:03Z", "updated_at": "2026-11-01T20:10:00Z", "model": "opus", "git_branch": "main"}
```

Only `id` is required. Other fields are strings or absent, except `last_message` (an optional
object with `role` and `text`) and `message_count` (an optional integer from 0 to 4294967295).
Existing commands may omit the new fields.

- `id` is what `terminal.resume_args` receives as `${session_id}`.
- `cwd`: with a `folder` request, rows whose `cwd` differs from it are dropped; omit `cwd` to
  mean the requested folder. With `folder: null`, rows need an absolute `cwd`.
- `created_at`/`updated_at` are RFC 3339. Binding needs `created_at`: a new session with no
  creation time is never bound to the terminal that started it.
- `last_message`: `{ "role": "user" | "agent", "text": "..." }` for the latest user or
  assistant message, whichever came last. Use `agent` for assistant messages.
  Include text blocks only, excluding system/meta records, tool payloads, and interruption
  markers. Axome collapses whitespace, trims, and caps the preview at 200 characters.
- `message_count`: number of meaningful user/assistant message records containing text;
  exclude system/meta records and tool-result/tool-only records. Omit if unavailable, not zero.
- `model`: the model used by the latest assistant turn, not the initial or configured default.
  Omit if unavailable.
- Repeated ids keep the row with the latest `updated_at`.

A **watch path**:

```json
{"watch": "/home/me/.foo/sessions"}
```

The path must be absolute. Axome re-lists this source when the path changes, or when one of its
direct children changes if it's a directory. A path that doesn't exist yet is watched from its
nearest existing ancestor until it appears. Report the paths this listing read from: the
folder's session directory for a folder request, the store root and each directory read for
`folder: null`, and the `-wal` file next to a SQLite database. Axome uses the paths from the
latest successful listing, re-lists folders at most every 2 seconds, and re-lists the Home
view at most every 30 seconds.

Watch paths are what keep a source live: a listing that reports none is re-listed only when its
folder is shown, when a session ends, or when settings change. If Axome can't watch a reported
path, the source shows "Live updates unavailable" and retries on its next listing.

Lines that are neither, including relative watch paths, are skipped. The folder's history then
shows how many entries couldn't be read, and the rest still list.

### Failure

Exit non-zero to fail the listing: Axome keeps the last good sessions and shows the last
non-empty stderr line. Don't exit non-zero for one unreadable session; skip it instead. An
empty, successful listing clears the source.
