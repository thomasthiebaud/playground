# buck2-go-rules

Write custom Buck2 rules to build Go programs — from scratch, no prelude magic.

By the end of this project you will understand how Buck2's rule system works (providers, attrs, actions, toolchains) and have a working `go_binary` / `go_library` / `go_test` rule set that can build real Go code.

## Prerequisites

- Go installed and on your `$PATH` (`go version` should print ≥ 1.21)
- Buck2 working from the repo root (`./buck2 --version`)
- Read the [Buck2 rule authoring docs](https://buck2.build/docs/rule_authors/writing_rules/) at least once

## Exercises

### Exercise 0 — Hello genrule

**Goal:** Confirm Buck2 can shell out to `go build` via a plain `genrule`.

**Requirements:**
1. Create a minimal Go source file (`main.go`) in `projects/buck2-go-rules/src/hello/` that prints `"hello from buck2"`.
2. Add a `genrule` target in this project's `BUCK` file that:
   - Takes the Go source file as input (`srcs`)
   - Runs `go build` to produce a binary
   - Copies the binary to `$OUT`
3. Build and run it.

**Verify:**
```bash
./buck2 build //projects/buck2-go-rules:hello_genrule
./buck2 run //projects/buck2-go-rules:hello_genrule
# Should print: hello from buck2
```

**Why this matters:** `genrule` works but is a black box — Buck2 can't reason about Go-specific inputs/outputs, so caching and dependency tracking are coarse. This motivates writing a real rule.

---

### Exercise 1 — Your first custom rule: `go_binary` (v1)

**Goal:** Replace the `genrule` with a custom `go_binary` rule that calls `go build` via a Buck2 action.

**Requirements:**
1. Create a `.bzl` file (e.g. `go_rules.bzl`) inside `projects/buck2-go-rules/`.
2. Define a `go_binary` rule that:
   - Accepts a `name` (string) and `srcs` (list of source files) attribute.
   - In its `impl` function, runs `go build` using `ctx.actions.run()` to produce a binary.
   - Returns a `DefaultInfo` provider with the binary as the default output.
   - Returns a `RunInfo` provider so `buck2 run` works.
3. Load and use this rule in your `BUCK` file.

**Key concepts to research:**
- `rule()` function signature — [Writing Rules](https://buck2.build/docs/rule_authors/writing_rules/)
- `ctx.actions.run()` — how to invoke external commands
- `ctx.actions.declare_output()` — how to declare build outputs
- `DefaultInfo` and `RunInfo` providers
- `cmd_args()` — how to build command lines

**Verify:**
```bash
./buck2 build //projects/buck2-go-rules:hello
./buck2 run //projects/buck2-go-rules:hello
# Should print: hello from buck2
```

**Hint:** Look at how `ctx.attrs.srcs` gives you a list of `Artifact` objects, and how `cmd_args()` can reference artifacts and output paths.

---

### Exercise 2 — `go_library` and `deps`

**Goal:** Support multi-package builds by adding a `go_library` rule and wiring up `deps`.

**Requirements:**
1. Create a Go package in `src/greeter/` that exports a `Greet(name string) string` function.
2. Update `src/hello/main.go` to import and use the greeter package.
3. Define a `go_library` rule in your `.bzl` file that:
   - Accepts `name`, `srcs`, `importpath` (string), and `deps` (list of deps) attributes.
   - Compiles the package using `go build` or `go tool compile` (your choice).
   - Returns a provider (you'll need to define one, e.g. `GoLibraryInfo`) carrying the compiled artifact and import path.
4. Update `go_binary` to:
   - Accept a `deps` attribute.
   - Collect `GoLibraryInfo` from each dep.
   - Pass the right flags so Go can find the dependency packages at build time.

**Key concepts to research:**
- Custom providers — `provider()` function
- Accessing dependency providers — `ctx.attrs.deps` and iterating over them
- `go tool compile` and `go tool link` — the lower-level Go build commands
- `-importcfg` flag for controlling Go's package resolution

**Verify:**
```bash
./buck2 build //projects/buck2-go-rules:hello
./buck2 run //projects/buck2-go-rules:hello
# Should print: Hello, Buck2! (or whatever your greeter returns)
```

**Hint:** You may find it simpler to start with `go tool compile` / `go tool link` rather than `go build`, since those give you explicit control over import paths and output locations that Buck2 needs.

---

### Exercise 3 — `go_test`

**Goal:** Add a `go_test` rule so `buck2 test` works for Go code.

**Requirements:**
1. Write a test file for the greeter package (`greeter_test.go`).
2. Define a `go_test` rule that:
   - Accepts `name`, `srcs`, `deps` attributes.
   - Compiles and runs the test using `go test` or the lower-level compile/link/run approach.
   - Integrates with Buck2's test runner via `ExternalRunnerTestInfo` provider.
3. The test target should be runnable via `buck2 test`.

**Key concepts to research:**
- `ExternalRunnerTestInfo` provider — how Buck2 discovers and runs tests
- Test result reporting — how Buck2 expects test pass/fail signals
- `go test -v` output format vs. what Buck2's test runner expects

**Verify:**
```bash
./buck2 test //projects/buck2-go-rules:greeter_test
# Should report test results (pass/fail)
```

---

### Exercise 4 — Toolchain rule

**Goal:** Stop hard-coding the Go binary path. Define a proper Buck2 toolchain so the Go compiler is resolved once and shared across all rules.

**Requirements:**
1. Define a `GoToolchainInfo` provider that carries the path to the `go` binary (and optionally `compile`, `link`, `asm` tool paths).
2. Create a `go_toolchain` rule that:
   - Detects or is configured with the Go installation path.
   - Returns `GoToolchainInfo`.
3. Register it in `toolchains/BUCK`.
4. Update all your `go_*` rules to obtain the toolchain from `ctx.attrs._go_toolchain` (toolchain dep attribute) instead of hard-coding `"go"`.

**Key concepts to research:**
- Toolchain rules — [Buck2 Toolchains](https://buck2.build/docs/rule_authors/writing_rules/#toolchains)
- `attrs.toolchain_dep()` — how rules reference toolchains
- `attrs.default_only()` — hiding toolchain attrs from users

**Verify:**
```bash
# All previous exercises should still work
./buck2 build //projects/buck2-go-rules:hello
./buck2 run //projects/buck2-go-rules:hello
./buck2 test //projects/buck2-go-rules:greeter_test
```

---

### Exercise 5 — Caching and incrementality

**Goal:** Validate that Buck2's caching works correctly with your rules.

**Requirements:**
1. Build `hello` twice — the second build should be a no-op (check Buck2's output for cache hits).
2. Change only `greeter.go`, rebuild — only the greeter library and the final binary should rebuild, NOT an unrelated target.
3. Add a second binary target (`src/cli/main.go`) that does NOT depend on greeter. Change `greeter.go` again and confirm the CLI binary is NOT rebuilt.

**Verify:**
```bash
./buck2 build //projects/buck2-go-rules:hello
# Change greeter.go
./buck2 build //projects/buck2-go-rules:hello
# Observe from buck2 log that only greeter + hello were re-built

./buck2 build //projects/buck2-go-rules:cli
# Change greeter.go
./buck2 build //projects/buck2-go-rules:cli
# cli should be a cache hit (it doesn't depend on greeter)
```

**Hint:** `./buck2 log what-ran` shows which actions actually executed.

---

## Stretch Goals

- **Cross-compilation:** Add `goarch` / `goos` attrs to `go_binary` and set `GOARCH`/`GOOS` env vars in the build action.
- **CGo support:** Handle `.c` files mixed with `.go` files.
- **`go_module` / `go_vendor`:** Fetch third-party modules and make them available as Buck2 deps.

## References

- [Buck2 Documentation](https://buck2.build/docs/)
- [Writing Rules](https://buck2.build/docs/rule_authors/writing_rules/)
- [Buck2 API — Starlark Builtins](https://buck2.build/docs/api/starlark/)
- [Go Toolchain Internals](https://pkg.go.dev/cmd/go) — `go tool compile`, `go tool link`
- [Buck2 GitHub Examples](https://github.com/facebook/buck2/tree/main/examples)
