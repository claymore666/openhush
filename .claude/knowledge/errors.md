# Past Errors & Mitigations

This document tracks bugs, mistakes, and their solutions to prevent recurrence.

---

## Format

```markdown
### ERR-XXX: Short Title
**Date**: YYYY-MM-DD
**Severity**: Critical | High | Medium | Low
**Category**: Logic | Security | Performance | UX

**What happened**:
Description of the error.

**Root cause**:
Why it happened.

**Mitigation**:
How we fixed it.

**Prevention**:
How to prevent recurrence (added to sanity checks).
```

---

## Errors

*(None yet — this file will grow as we encounter and fix issues)*

### ERR-001: Template Entry
**Date**: 2024-XX-XX
**Severity**: Medium
**Category**: Logic

**What happened**:
[Description]

**Root cause**:
[Why]

**Mitigation**:
[Fix]

**Prevention**:
- [ ] Added check to sanity agent
- [ ] Added test case
- [ ] Updated documentation

---

## Error Categories

| Category | Description | Example |
|----------|-------------|---------|
| Logic | Incorrect behavior | Wrong output order |
| Security | Vulnerability | Input injection |
| Performance | Slow or resource-heavy | Memory leak |
| UX | Poor user experience | Confusing error message |
| Concurrency | Race conditions | Data corruption |
| Platform | OS-specific issues | Wayland paste fails |
