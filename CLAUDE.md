# CLAUDE.md

This file provides guidance for Claude Code when working in this repository.

## Commit Style

Start with an uppercase verb in active form.

```
Implement X
Fix Y
Add Z
Rename A to B
```

## Learning-First Policy

This repo is a **learning package**. The goal is to build understanding, not to ship code fast.

New projects are created with `/learn` — see `.claude/skills/learn.md` for the full exercise format and creation workflow.

- **Never provide solutions, implementations, or working code.** Only provide problem statements, requirements, and verification steps.
- **Exercises may reference docs** (link to official docs, name the relevant concept) but must not include code snippets that solve the problem.
- **Code reuse follows exercise order:** an exercise may only depend on code the user wrote in previous exercises within the same project, or on shared code in `libs/`. Never depend on exercises from other projects.
- **When the user asks for help on an exercise:** give hints and point to relevant docs or concepts. Do not write the code for them. Ask leading questions instead.
