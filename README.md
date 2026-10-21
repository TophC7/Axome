# Sworm <img src="src-tauri/icons/icon.svg" width="36" height="36" align="top" alt="" />

**A Linux desktop workspace for coding-agent CLIs.**

Keep using Claude Code, Codex, OMP, or Antigravity in the terminal. Sworm puts a file editor, Git diffs, and local issues alongside them, so you can follow the work and review changes without bouncing between separate apps.

Tabs can belong to different repositories. Switch tabs, and the file tree and Git sidebar follow.

Sworm is a passion project in active development, built around a simple idea: keep the agent's own interface, and make the work around it easier.

![Sworm in action: switching tabs, editing files, reviewing diffs, and launching agents](docs/readme/tab-driven-workflow.webp)

## Try it

With [Nix](https://nixos.org/) and flakes enabled:

```sh
nix run github:tophc7/Sworm
```

Install and authenticate whichever agent CLI you want to use separately. Sworm detects supported installed agents; you can also open a regular terminal using your login shell.

<details>
<summary>Install on Debian, Fedora, or Arch</summary>

Packages use your distribution's GTK, WebKitGTK, and graphics drivers.

### Debian

Download the `.deb` for your architecture from [GitHub Releases](https://github.com/TophC7/Sworm/releases/latest), then install:

```sh
sudo apt install ./sworm_*_$(dpkg --print-architecture).deb
```

Available for Debian 13+ and Ubuntu 24.04+ on `amd64` and `arm64`. To update, install a newer release with the same command.

### Fedora

Download the `.rpm` for your architecture from [GitHub Releases](https://github.com/TophC7/Sworm/releases/latest), then install:

```sh
sudo dnf install ./sworm-*-1.$(uname -m).rpm
```

Supports Fedora 40+ on `x86_64` and `aarch64`. To update, install a newer release with the same command.

### Arch

Download `sworm-bin-<commit>.tar.gz` from [GitHub Releases](https://github.com/TophC7/Sworm/releases/latest) into an empty directory, then unpack and install:

Using `yay`:

```sh
tar -xf sworm-bin-*.tar.gz
yay -Bi .
```

Using `makepkg`:

```sh
sudo pacman -S --needed base-devel
tar -xf sworm-bin-*.tar.gz
makepkg -si
```

Supports `x86_64` and `aarch64`.

</details>

## Why I built Sworm

I've always liked VS Code's straightforward workflow. I was much less happy with it's use of Electron, especially on Wayland, and wanted something that felt familiar without relying on it.

As coding agents became part of my daily work, I found I really only needed a file tree, Git diffs, an editor, and a terminal. I wasn't using any of the rest of the "IDE" or its extensions anymore; but VS Code's workflow was still the only that worked for me.

Sworm grew out of that: a workspace for the parts I actually use when working with agents. It's evolved into a tab-driven workflow of its own, while keeping enough familiarity that VS Code users should feel at home.

## What you can do

### Keep agents and repositories in reach

Run agent sessions and ordinary terminals in tabs, with each tab tied to its folder. The sidebar follows the selected tab, and a folder stays open until its last tab closes. Open multiple windows and drag tabs between them.

The home dashboard reads local Claude Code, Codex, and OMP histories to show projects with a seven-day activity heatmap, so you can find where you left off.

<details>
<summary>📷 See the dashboard and an agent workspace</summary>

![Project dashboard with seven-day agent activity](docs/readme/home.png)

![An OMP session beside Sworm's Git changes and commit graph](docs/readme/git-gragh-diff_omp-session.png)

</details>

### Edit and review the work

Browse files, edit code, and review changes before committing:

- **Side-by-side or unified Git diffs** with hunk actions, commit and working-tree comparisons, and file or clipboard compare.
- **Full Source Control workflow** with staged and working changes, commit variants (**Commit & Push**, **Commit & Sync**, **Amend**, **Commit All**, **Sign Off**, **Skip Hooks**, **Undo Last Commit**), and branch sync.
- **Merge, rebase, cherry-pick, and conflict resolution** with paused-operation controls (**Continue**, **Skip**, **Abort**), whole-file or per-block resolution in Monaco (**Accept Current**, **Accept Incoming**, **Accept Both**), and conflict navigation.
- **Interactive Git graph and ref management** across **Branches**, **Tags**, **Stashes**, and **Worktrees** tabs—check out, cherry-pick, revert, reset, manage remotes, or **Stash & Checkout** when switching with uncommitted edits.
- **Monaco code editing** with built-in language server (LSP) support (Rust, TypeScript/JavaScript, Svelte, Python, Nix, Fish, HTML, CSS, Tailwind, Biome) and Shiki highlighting.
- **Markdown editing** with a live preview styled to match GitHub as closely as possible.
- **File tree** with filtering, drag-and-drop, status-aware Git actions, and safe deletion/discard to the system trash.
- **Runnable project tasks** defined in `.sworm/tasks.jsonc`.

<details>
<summary>📷 See Git diffs, the code editor, Markdown preview, and command palette</summary>

![Sworm showing a split commit diff beside the file changes and Git graph](docs/readme/commit-diff.png)

![Code editing in Sworm](docs/readme/monaco-file.png)

![Markdown source and live preview in Sworm](docs/readme/markdown-split.png)

![Workbench command palette](docs/readme/pallete.png)

</details>

Use `Ctrl+Shift+P` for workbench commands, `Ctrl+P` to find files, or `Ctrl+O` to open a folder or remote host. The command palette also has modes for editor commands (`>`), runnable tasks (`!`), and files (`/`), and exposes the full set of `Git` actions. Shortcuts are customizable.

### Isolate work with Git worktrees

Because tabs in the same folder share its working tree, you can spin up separate Git worktrees when running multiple agents in parallel. Create, open, and remove worktrees from the Git sidebar's **Worktrees** tab, branch menus, or commit history, or use **Migrate Changes to Main** to move uncommitted work back to the primary checkout.

To copy untracked local files (such as `.env`) into new worktrees automatically, configure `git.worktree_copy` in `.sworm/settings.jsonc`:

```jsonc
{
  "git": {
    "worktree_copy": {
      ".env": true,
      ".local": true,
      ".local/private": false
    }
  }
}
```

Tasks in `.sworm/tasks.jsonc` can also run automatically when a worktree is created:

```jsonc
{
  "version": 1,
  "tasks": [
    {
      "id": "setup",
      "label": "Set up worktree",
      "command": "bun install",
      "runOn": "worktreeCreated",
      "confirm": true
    }
  ]
}
```

### Use your project's Nix environment

Sworm detects `flake.nix`, `shell.nix`, and `default.nix`. Select and evaluate an environment from the Nix applet at the foot of the sidebar rail; once ready, it is used for new terminals, agent sessions, runnable tasks, and language servers.

Everything Sworm launches starts from the environment Sworm itself was launched with, minus Sworm's own packaging edits (the GTK wrapper and devshell record each variable's prior value as `SWORM_ORIG_<NAME>`, and Sworm restores it). That environment is resolved through your login shell (`$SHELL -l`, including home-manager session variables), and the folder's Nix environment is layered on top; session values such as `DISPLAY`, `WAYLAND_DISPLAY`, or `SSH_AUTH_SOCK` always come from the host. After changing your login files, run **Reload Environment** from the command palette; new terminals, sessions, and tasks pick it up without restarting Sworm.

The sidebar rail's foot holds the Nix environment, remote servers, notifications and the app menu; the sidebar footer shows the current folder.

<details>
<summary>📷 See a terminal session</summary>

![Terminal session with the project environment status visible](docs/readme/terminal.png)

</details>

### Terminal presets

Define shell launch variants under `shell.presets` in `~/.config/sworm/settings.jsonc` or a folder's `.sworm/settings.jsonc`. Each preset can override the shell `command`, append `args`, add `env`, and type a `startup` line into the shell after it starts. `shell.default_preset` picks what **New Terminal** (Ctrl+T) opens; every preset also gets its own New Tab card and palette command.

```jsonc
{
  "shell": {
    "default_preset": "dev",
    "presets": {
      "dev": { "args": ["-l"], "env": { "RUST_LOG": "debug" }, "startup": "git status -sb" }
    }
  }
}
```

A task in `.sworm/tasks.jsonc` can run inside a preset with `"shell": "dev"`: the preset's `startup` runs first, then the task's `command`. Tasks without `shell` run under your login shell.

### Chat with agents

Harnesses with an ACP agent (omp today) can run a session tab as a **Chat** view instead of the terminal TUI: streamed markdown replies, compact tool-call rows with inline permission prompts and diffs, forms for the agent's questions, and model/mode/thinking pickers in the composer. **Switch to Terminal** (tab context menu, palette, or the composer button) stops the chat and resumes the same conversation in the TUI, and back again. Set `"view": "chat"` on a harness preset to open it in Chat by default. Chat tabs with the same preset in the same folder environment share one agent process, which exits a minute after its last chat closes.

Chat sessions get MCP servers only from Sworm: add stdio servers under `sessions.mcp_servers` in your global settings, and extensions can contribute more.

```jsonc
{
  "harnesses": { "omp": { "presets": { "chat": { "view": "chat" } } } },
  "sessions": { "mcp_servers": { "docs": { "command": "docs-mcp", "args": ["--stdio"] } } }
}
```

### Keep local tasks alongside the code

Track issues with priorities, group them into epics, and see progress in the sidebar. Agents can query, create, and update issues through a local socket API rather than editing the database directly.

<details>
<summary>📷 See the new tab page and issues sidebar</summary>

![New tab page beside the local issues sidebar](docs/readme/new-session.png)

</details>

### Work on remote hosts or in the browser

Run `sworm-server` on another Linux machine to open remote folders from the desktop app over QUIC (with mTLS fingerprint pairing), or enable its web listener to use the Sworm workbench directly in a browser. Agent sessions and tasks keep running on the server if you disconnect or close the window, and reattach when you return.

The flake exports `packages.<system>.sworm-server`, `nixosModules.sworm-server`, and `homeManagerModules.sworm` for declarative setups, alongside `.deb`, `.rpm`, and tarball server artifacts on GitHub Releases.

## Worth knowing

- **Active development, Linux-first.** Expect rough edges and changes as the project grows. I do daily-drive it however, so it's stable enough.
- **Sessions in the same folder share its working tree.** Tabs do not create isolated worktrees; create a separate worktree for independent edits.
- **Your agent is still your agent.** Sworm runs the CLI itself; model information and other output inside its terminal come from that CLI.

## Development

Nix provides the development toolchain and Linux desktop dependencies:

```sh
git clone https://github.com/tophc7/Sworm.git
cd Sworm
nix develop
bun install
bun run app:dev
```

Build and run the Nix package from the checkout:

```sh
nix build
nix run .
```

Sworm uses [Tauri v2](https://tauri.app/) and Rust for the desktop runtime, with the system Git CLI and SQLite. The interface is built with Svelte 5, SvelteKit, and Tailwind CSS v4; editing uses Monaco and Shiki, and terminals use xterm.js. Nix defines the development environment and packaging.

## Feedback

If you try Sworm, I'd like to hear what works for you and what gets in the way. [Open an issue](https://github.com/tophc7/Sworm/issues) with bugs, ideas, or how you use coding agents.

## Credits

- [Emdash](https://github.com/generalaction/emdash) was an early architectural inspiration for Sworm's agent-focused workspace.
- [VS Code](https://code.visualstudio.com/) shaped the workflow and UI familiarity I wanted to keep. Sworm also uses Microsoft's [Monaco Editor](https://microsoft.github.io/monaco-editor/), the editor behind VS Code.

## License

[AGPL-3.0-or-later](./LICENSE)
