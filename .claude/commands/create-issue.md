# Create Issue Agent

Create a GitHub issue from a feature request or bug description.

## Instructions

You are the Issue Creation Agent for OpenHush. Convert user requests into well-structured GitHub issues.

## Input

User description of feature or bug.

## Steps

1. **Determine issue type**:
   - Feature/Requirement → use requirement template
   - Bug → use bug template
   - Documentation → simple issue

2. **For Requirements**:

   a. Assign next available ID:
      - Read `REQUIREMENTS.md` to find highest Fxx, Pxx, Cxx, Dxx
      - Increment appropriate category

   b. Determine priority (ask user if unclear):
      - Must — Required for milestone
      - Should — Important but not blocking
      - Could — Nice to have

   c. Write acceptance criteria:
      - Specific, testable conditions
      - Include edge cases

   d. Write test cases:
      - Happy path
      - Edge cases
      - Error conditions

   e. Identify dependencies:
      - Which other requirements must be done first?

3. **For Bugs**:

   a. Extract reproduction steps
   b. Identify expected vs actual behavior
   c. Determine platform/environment
   d. Request logs if needed

4. **Output format**:

For requirements, output GitHub issue body:

```markdown
## Requirement ID
F14 (or appropriate ID)

## Priority
Should

## Category
Core Feature (F)

## Description
[Clear description of what the feature does and why it's needed]

## User Story
As a [user type],
I want [goal],
so that [reason].

## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Test Cases

### Happy Path
- Test: [description]
  - Input: [input]
  - Expected: [output]

### Edge Cases
- Test: [description]
  - Input: [edge case input]
  - Expected: [behavior]

### Error Cases
- Test: [description]
  - Trigger: [how to trigger error]
  - Expected: [error code and message]

## Dependencies
- F01: Background daemon (must be running)
- F02: Hotkey trigger

## Technical Notes
- Affects: `src/module.rs`
- New types needed: [if any]
- Breaking change: No
```

5. **Create the issue**:
   ```bash
   gh issue create --title "[REQ] F14: Feature Title" --body "..." --label "requirement,triage"
   ```

6. **Update REQUIREMENTS.md**:
   - Add new row to appropriate table
   - Keep IDs sequential

7. **Assign to milestone** (if specified by user)
