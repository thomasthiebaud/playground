# CLAUDE.md

This file provides guidance for Claude Code when working in this repository.

## Commit Style

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]
```

Common types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`

## Playground Monorepo

Each experiment lives in `projects/<name>/` and is fully self-contained:

- `BUCK` — Build targets (rust_binary, cxx_binary, etc.)
- `src/` — Source code
- `Dockerfile` — Container image for the built artifact
- `docker-compose.yml` — Runtime dependencies (Kafka, Postgres, etc.) and the service itself
- `README.md` — Design decisions, learnings, and context

Projects may depend on shared code in `libs/` but never on each other. Buck2 builds, Docker runs.

When adding a new project, update the root `README.md` with a short description and link to the project folder. The root README serves as the portfolio landing page.

## Learning-First Policy

This repo is a **learning package**. The goal is to build understanding, not to ship code fast.

- **Never provide solutions, implementations, or working code.** Only provide problem statements, requirements, and verification steps.
- **Exercises must include clear acceptance criteria** — how the user knows they got it right (e.g., "verify with `curl ...`", "benchmark should show X").
- **Exercises may reference docs** (link to official docs, name the relevant concept) but must not include code snippets that solve the problem.
- **Code reuse follows exercise order:** an exercise may only depend on code the user wrote in previous exercises within the same project, or on shared code in `libs/`. Never depend on exercises from other projects.
- **When the user asks for help on an exercise:** give hints and point to relevant docs or concepts. Do not write the code for them. Ask leading questions instead.
