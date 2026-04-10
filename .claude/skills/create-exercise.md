# Create Exercise

Create a structured learning exercise project in this monorepo.

You have been given a clear spec:
- What to build (the final deliverable)
- The user's starting point (languages/tools they know)
- What's off-limits (libraries/abstractions to avoid)

## Instructions

### Step 1 — Design the exercises

Design just enough exercises to cover the topic end-to-end — no more, no less. Each exercise builds directly on the previous one. No exercise may depend on code from a later one.

Each exercise must have:
- A clear, single goal
- A prose intro explaining why it matters and what will be achieved
- A checklist of concrete steps (no solutions, no working code)
- Verify steps with exact commands the user can run to confirm success
- Hints if the exercise involves non-obvious tooling

Steps and **Verify:** must not overlap:
- Steps = actions (write, add, implement, edit files)
- **Verify:** = commands to run to confirm success (`bazel test`, `bazel run`, `curl`, etc.)
- Never put a verification command as a checklist step

Each exercise must be tightly scoped:
- Introduce exactly one new concept or skill — nothing else
- Do not require knowledge that hasn't been covered in a previous exercise
- If a step needs a prerequisite concept, that concept must have its own exercise first

### Step 2 — Create the project

1. Choose the next available project number (`NNN_name`) and create `projects/NNN_name/` with:
   - `BUILD` — Bazel build targets (at minimum a stub target)
   - `src/` — empty source directory
   - `README.md` — the exercise notebook (see format below)

2. Update the root `README.md` — add a row to the projects table with a short description and link

## README format

```
# <project-name>

<one paragraph: what will be built and what concepts will be learned>

**Language:** <language(s)>

**Install deps before starting:**
<pip install / cargo add / etc. — only if needed>

## Prerequisites

- [ ] <prerequisite with verification command>

---

## Exercise 0 — <name>

<prose intro: why this exercise matters, what will be achieved>

- [ ] <step>
- [ ] <step>

**Verify:**
- [ ] <command and expected output>

**Hint:** <only if non-obvious tooling is involved>

---

## Exercise N — ...
```

## Rules (from CLAUDE.md)

- Never provide solutions, implementations, or working code
- Exercises may reference docs (link to official docs, name the relevant concept) but must not include code snippets that solve the problem
- Verify steps must be concrete and runnable

## Build system

All exercises use Bazel as the entry point — `bazel run`, `bazel build`, `bazel test`.

- **Rust:** hermetic toolchain downloaded automatically; add crates to `third-party/rust/Cargo.toml`, run `CARGO_BAZEL_REPIN=1 bazel build <target>`
- **Python:** pip-installed on the host; genrule targets that call `python3` must have `tags = ["no-sandbox"]` in the BUILD file
