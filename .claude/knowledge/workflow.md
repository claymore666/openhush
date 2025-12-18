# Development Workflow

---

## ⚠️ CRITICAL: No AI Attribution Rule

**All commits, PRs, releases, and public-facing artifacts must NOT include:**

- "Generated with Claude Code" or similar
- "Co-Authored-By: Claude" or any AI assistant
- Any mention of AI tools in commit messages
- AI attribution in release notes or changelogs

This applies to:
- Git commits (no co-author trailers)
- Pull request descriptions
- Release notes
- Package metadata
- README credits

**Rationale**: The human developer takes full responsibility for all code. AI assistance is a tool, not an author.

---

## Branching Strategy

```
main ← stable releases only
  ↑
  PR (after all CI + agents pass)
  │
release/v0.1.0 ← milestone branch
  ↑
  branch when milestone complete
  │
dev ← integration branch (default)
  ↑
  PR from feature branches
  │
feature/F13-queued-dictation
fix/123-some-bug
```

### Branch Rules

| Branch | Who merges | Requirements |
|--------|------------|--------------|
| `main` | Human only | All CI green, release tested, changelog updated |
| `release/*` | Human only | Milestone complete, all issues closed |
| `dev` | Human (after agent approval) | CI green, sanity agent pass, tests pass |
| `feature/*` | Via PR to dev | Same as dev |

### Branch Naming

- `feature/F13-short-description` — New features (linked to requirement ID)
- `fix/123-short-description` — Bug fixes (linked to issue number)
- `refactor/area-description` — Code improvements
- `docs/topic` — Documentation only
- `release/vX.Y.Z` — Release preparation

## Versioning

**Semver with 0.x freedom**

- `0.x.y` — Breaking changes allowed, move fast
- `1.0.0+` — Strict semver, breaking = major bump

```
0.1.0 — MVP
0.2.0 — Queued dictation
0.3.0 — Multi-GPU
...
1.0.0 — Stable release (Linux + macOS + Windows)
```

## Release Process

1. All milestone issues closed
2. Branch `release/vX.Y.Z` from `dev`
3. Update version in `Cargo.toml`
4. Update `CHANGELOG.md`
5. Run full test suite + manual testing
6. PR `release/vX.Y.Z` → `main`
7. After merge: tag `vX.Y.Z`
8. GitHub Actions builds packages, creates release
9. Merge `main` back to `dev` (fast-forward)

## Issue Workflow

```
Open → Triaged → In Progress → Review → Done
```

| Status | Meaning |
|--------|---------|
| Open | New, needs triage |
| Triaged | Assigned to milestone, ready to work |
| In Progress | Someone working on it |
| Review | PR open, awaiting review |
| Done | Merged to dev |

## PR Workflow

1. Create branch from `dev`
2. Implement + write tests
3. Run local checks (`cargo fmt`, `cargo clippy`, `cargo test`)
4. Push, create PR to `dev`
5. CI runs (GitHub Actions)
6. Sanity Agent reviews
7. Claude Code reviews agent feedback, fixes issues
8. Loop until CI + agents pass
9. Human reviews and approves
10. Squash merge to `dev`

## Architecture Review

**On every PR that touches:**
- Module structure (`mod.rs` files)
- Public interfaces (trait definitions)
- Data flow (channels, queues)
- New dependencies

**Reviewer must:**
1. Check if architecture.md needs update
2. Check if interfaces.md needs update
3. Update docs in same PR

## Documentation

| Type | Location |
|------|----------|
| User docs | GitHub Wiki |
| API docs | `cargo doc` (rustdoc) |
| Architecture | `.claude/knowledge/` |
| Changelog | `CHANGELOG.md` |

## Debug Info for Bug Reports

Users run:
```bash
openhush debug-info > debug.txt
```

Outputs:
- Version
- OS / display server
- GPU info
- Config (sanitized)
- Recent errors from log

Attach to GitHub issue.

## Sprint/Milestone Cadence

**Milestone-based (not time-boxed)**

1. Define milestone scope (issues)
2. Work until all issues done
3. Release when ready
4. Retrospective: what went well, what to improve

---

## Definition of Done

A feature/issue is **DONE** when ALL of the following are true:

### Code Complete
- [ ] Implementation matches acceptance criteria
- [ ] All new code has tests (unit + integration where applicable)
- [ ] Tests pass locally (`cargo test`)
- [ ] No new clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt`)
- [ ] No security issues (`cargo audit`)

### Agent Approved
- [ ] Sanity agent passes (no anti-patterns, security issues)
- [ ] Test coverage adequate (test agent review)
- [ ] Architecture docs updated if structure changed

### Documentation
- [ ] Public APIs have rustdoc comments
- [ ] Error codes documented (if new errors added)
- [ ] CHANGELOG.md updated (for user-visible changes)
- [ ] README updated (if CLI or config changed)

### Human Signoff
- [ ] Human reviewed and approved
- [ ] PR merged to `dev`

### For Releases Only (additional)
- [ ] All milestone issues Done
- [ ] Version bumped in Cargo.toml
- [ ] Release notes written
- [ ] Tested on target platforms
- [ ] PR merged to `main`
- [ ] Git tag created

---

## Checklist for Human Review

Since agents handle code quality, human review focuses on:

1. **Intent** — Does this solve the right problem?
2. **UX** — Is the user experience good?
3. **Scope** — Is it doing too much or too little?
4. **Edge cases** — Any scenarios not considered?
5. **Architecture fit** — Does it align with overall design?

---

## Technical Debt Tracking

When taking shortcuts:

1. Create issue with label `tech-debt`
2. Document in code with `// TODO(tech-debt#123): description`
3. Link to issue number
4. Review debt at each milestone retrospective

---

## Hotfix Process

If `main` breaks in production:

1. Branch `hotfix/vX.Y.Z` from `main`
2. Fix the issue (minimal change)
3. PR directly to `main` (skip `dev` for speed)
4. Tag new patch version
5. Cherry-pick fix back to `dev`
6. Post-mortem: add to errors.md

---

## Context Handoff

If Claude session ends mid-task:

**State is preserved in:**
- Todo list (current progress)
- Git status (uncommitted changes)
- `.claude/metrics/agent-log.md` (recent agent activity)
- GitHub issues (what's being worked on)

**To resume:**
Tell next Claude: "Continue work on issue #X" — it can read all context from files.

---

## Progress Reporting

During long tasks, Claude will:
- Update todo list as items complete
- Commit working increments (if appropriate)
- Summarize progress every ~5 significant steps
- Ask questions rather than assume
