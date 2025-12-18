# Meta Review Agent

Analyze agent performance and improve prompts.

## Instructions

You are the Meta Review Agent for OpenHush. Analyze how other agents performed and suggest improvements.

## Trigger

- Weekly (scheduled)
- After milestone completion
- On request

## Steps

1. **Read agent logs**:
   - `.claude/metrics/agent-log.md` — All agent activity

2. **Analyze sanity-check agent**:

   a. Count issues found:
      - True positives (real issues fixed)
      - False positives (flagged but not real issues)
      - False negatives (bugs found later that agent missed)

   b. Categorize by type:
      - Security issues caught
      - Anti-patterns caught
      - Performance issues caught
      - Missed patterns

   c. Calculate effectiveness:
      - Precision = TP / (TP + FP)
      - Recall = TP / (TP + FN)

3. **Analyze test-generator agent**:

   a. Test quality:
      - Tests that caught real bugs
      - Tests that were redundant
      - Missing test cases (bugs in untested paths)

   b. Coverage:
      - Are generated tests actually covering the requirement?
      - Edge cases missed?

4. **Identify improvement opportunities**:

   For each agent, answer:
   - What patterns is it missing?
   - What is it over-flagging?
   - What context would help it perform better?

5. **Update agent prompts**:

   If improvements identified:
   - Edit `.claude/commands/sanity-check.md`
   - Edit `.claude/commands/generate-tests.md`
   - Add new patterns to knowledge base

6. **Update knowledge base**:

   - Add missed patterns to `anti-patterns.md`
   - Add new error types to `errors.md`
   - Add new test patterns to `test-patterns.md`

7. **Output format**:

```markdown
## Meta Review Report — [Date]

### Agent Performance Summary

| Agent | Reviews | True Pos | False Pos | False Neg | Precision | Recall |
|-------|---------|----------|-----------|-----------|-----------|--------|
| sanity-check | 15 | 12 | 2 | 1 | 85% | 92% |
| test-generator | 8 | - | - | - | - | - |

### Sanity Check Agent

#### What's Working
- Catching unwrap() usage: 100% accuracy
- Security pattern detection: Good

#### What Needs Improvement
- Missing: [pattern] — add to anti-patterns.md
- Over-flagging: [pattern] — refine criteria

#### Prompt Changes Made
```diff
- Check for unbounded allocations
+ Check for unbounded allocations in hot paths (loops, handlers)
```

### Test Generator Agent

#### What's Working
- Unit test generation: Good coverage

#### What Needs Improvement
- Missing edge case: [description]
- Test pattern to add: [description]

### Knowledge Base Updates

| File | Change |
|------|--------|
| anti-patterns.md | Added: [pattern] |
| errors.md | Added: ERR-XXX |
| test-patterns.md | Added: [pattern] |

### Action Items
- [ ] Update sanity-check.md prompt
- [ ] Add pattern to anti-patterns.md
- [ ] Review next week
```

8. **Log this review** to agent-log.md
