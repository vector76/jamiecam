# Test-Driven Development (TDD)

This project follows Test-Driven Development principles, where **tests are central to the validation of the application**. 

- Tests should be written before or alongside implementation code
- All features must have corresponding tests
- Tests serve as the primary validation mechanism for correctness
- Code changes should be validated through the test suite
- The test suite should be comprehensive and cover edge cases

When implementing features or making changes:
1. Write or update tests first (or in parallel with implementation)
2. Ensure all tests pass before considering work complete
3. Use tests to guide the design and validate behavior
4. Maintain high test coverage

**Test execution strategy:** During development, run only tests relevant to the feature being worked on. Run the full test suite before commits.

**Shell commands:** Avoid using `cd` in commands. Use `pwd` to check the current directory if unsure.

# Git tips
- When committing, NEVER mention Claude Code as a coauthor or contributor
- AVOID `git -C ...` in commands. Instead, use `pwd` to check the current directory if unsure.

# Documentation

The `docs/` folder contains topic-oriented markdown documents organized in a **flat structure** (no subfolders). These documents serve as timeless knowledge base for the project.

**Purpose:**
- Document program architecture and design decisions
- Explain implementation choices and rationale
- Provide reference material for developers and agents
- Maintain permanent, stable documentation

**Guidelines:**
- ✅ Focus on architecture, design patterns, and implementation choices
- ✅ Prefer timeless documents; it's also OK to include design/implementation plans when they are useful context for the project
- ✅ Use descriptive filenames (e.g., `application-purpose.md`, `data-model.md`)
- ❌ Do NOT create subfolders (keep structure flat)
