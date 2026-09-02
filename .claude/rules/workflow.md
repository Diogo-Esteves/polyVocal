# Working style and human-in-the-loop workflow

## Finding out what to work on next

`docs/ROADMAP.md` is the checklist of phases/features, but **checkboxes can
go stale** — a past session found "Language auto-detection" marked
unchecked when it was in fact fully implemented end-to-end. Before treating
an unchecked roadmap item as real work, grep the code for it first; before
reporting one as done, verify the roadmap text still matches reality. The
`/roadmap-check` command automates this cross-check.

The real granular task backlog is `gh issue list`, not just ROADMAP.md
checkboxes — issues carry the specifics (repro steps, file:line, design
options) that the roadmap's phase-level checkboxes don't.

## General working style

- Prefer direct, in-place edits for normal-sized changes (a handful of
  files, one concern) over spinning up a separate worktree — reserve
  worktrees/parallel agents for genuinely independent, large-scope work.
- Feature branches + PR, rebase-and-merge on GitHub (branch names so far:
  `feat/...`, `chore/...`; PR merges rewrite commit hashes on `main` — if a
  stacked branch stops appearing merged after another PR lands, that's why,
  not a sign work was lost).
- After a task's PR merges (or work is abandoned), clean up: `git worktree
  remove <path>`, delete the now-merged branches locally and remotely, and
  fast-forward the main checkout. This applies to worktrees created directly
  in this session or via subagent isolation (`.claude/worktrees/agent-*` paths)
  — both are easy to forget since they're outside the main working directory.
- Run `cargo fmt`, `cargo clippy -D warnings`, and the full test suite
  (see `commands.md`) before calling backend work done — all three are
  cheap and CI enforces them anyway. The `/precommit` command runs the
  full set for both crates.

## Human-in-the-loop workflow

Tasks are scoped against `docs/ROADMAP.md` and `gh issue list` (verified
against real code first, per above) with the user. Once a task is fully
specified — exact files, exact behavior, exact tests — mechanical
implementation can be dispatched to a subagent instead of done directly:

- **`implementer`** (`.claude/agents/implementer.md`, Haiku) — a specced
  feature/bugfix that follows an established pattern in the codebase.
  Reserve it for specced, mechanical work; judgment calls (architecture,
  scope, ambiguous requirements) get reasoned through directly, not
  delegated to it.
- **`chore`** (`.claude/agents/chore.md`, Haiku) — mechanical work that
  isn't really "implementing a feature": renames, doc edits, dependency
  bumps, RUSTSEC advisory triage, issue/label hygiene.

The user does QA on the result. Git operations (commit, push, branch, PR)
stay with the orchestrating agent — neither subagent ever touches git.
