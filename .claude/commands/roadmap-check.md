---
description: Verify a docs/ROADMAP.md checkbox against real code before treating it as work to do (or as done)
argument-hint: <roadmap item or keyword, e.g. "language auto-detection">
allowed-tools: Read, Grep, Bash(git log:*)
---

Checkboxes in `docs/ROADMAP.md` go stale in both directions — items marked
unchecked that are already fully implemented, and items marked checked
that no longer match what the code does. Before reporting on the status
of **$ARGUMENTS**, do not trust the checkbox alone:

1. Read the exact line(s) in `docs/ROADMAP.md` matching $ARGUMENTS.
2. Grep the codebase (`src-tauri/src/`, `src-tauri/tests/`, `src/src/`) for
   the feature by its likely names — function names, command names, UI
   strings — not just the roadmap's phrasing.
3. If code exists, read enough of it to confirm it actually does what the
   roadmap line claims, not just that a same-named function exists.
4. Report: what the checkbox says, what the code actually does, and
   whether they agree. If they disagree, say so explicitly rather than
   silently trusting one over the other — flag it as a place `docs/ROADMAP.md`
   itself should be corrected, but don't edit the file unless asked.
