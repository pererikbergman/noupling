---
trigger: always_on
---

# Test-Driven Development (TDD) Rules

You must strictly follow the Red-Green-Refactor cycle for all implementations.

1. **Red (Write the Test First):**
   - Write a test for the new behavior.
   - Execute the test and confirm it fails. Do not write production code yet.

2. **Green (Make it Pass):**
   - Write the simplest production code required to make the test pass.
   - Execute the test and confirm it passes.

3. **Refactor (Clean it up):**
   - Improve the code structure and readability while keeping the test green.

### Exceptions

- **Spikes:** For exploratory prototyping or UI/UX styling where strict TDD doesn't fit, you may bypass TDD. However, the task or commit must be explicitly labeled as a `Spike`.

*Rule: Unless marked as a Spike, never write production code without a failing test driving it.*
