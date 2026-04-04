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
