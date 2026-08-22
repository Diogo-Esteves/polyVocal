---
description: Fetch a GitHub issue and classify it as chore-shaped, implementer-shaped, or needing a design decision first
argument-hint: <issue number>
allowed-tools: Bash(gh issue view:*), Read, Grep
---

Fetch issue **$ARGUMENTS** with `gh issue view $ARGUMENTS --comments` and
classify it per `.claude/rules/workflow.md`'s human-in-the-loop split:

- **`chore`** — mechanical, obvious-result work with no architectural
  judgment call: a dependency bump, a doc/README fix, a RUSTSEC advisory
  that's either not actionable or has a known upgrade path, label/issue
  hygiene. Read `.claude/agents/chore.md`'s scope before deciding this.
- **`implementer`** — the fix/feature is fully specced (exact files, exact
  behavior) and follows an established pattern already in the codebase.
  If the issue itself isn't specific enough to hand off as-is, say what's
  missing rather than writing the spec yourself unasked.
- **Needs a design decision first** — the issue names multiple plausible
  approaches (a UI interaction pattern, a scoping question, an
  architecture choice) and picking one is a judgment call, not a lookup.
  Summarize the options the issue already raises; don't pick one for the
  user.

Report: the issue's title and one-line summary, which category it falls
into and why, and — for chore/implementer — whether it's ready to dispatch
as-is or needs one clarifying question first.
