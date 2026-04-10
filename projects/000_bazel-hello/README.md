# bazel-hello

Verify the Bazel setup and get comfortable with the basics before starting the real projects.

## Prerequisites

- [x] Bazel installed: `bazel version` prints a version number
```bash
bazel version                                       
Bazelisk version: 1.28.1
Build label: 9.0.2
Build target: @@//src/main/java/com/google/devtools/build/lib/bazel:BazelServer
Build time: Thu Apr 09 22:08:10 2026 (1775772490)
Build timestamp: 1775772490
Build timestamp as int: 1775772490
```
- [x] Repo root is your working directory for all `bazel` commands

---

## Exercise 1 — Run the existing targets

The project already has a binary, a library, and tests. Run them to confirm your setup works.

- [x] `bazel run //projects/000_bazel-hello:hello` — prints `Hello, Bazel!`
```
bazel run //projects/000_bazel-hello:hello
INFO: Analyzed target //projects/000_bazel-hello:hello (2 packages loaded, 176 targets configured).
INFO: Found 1 target...
Target //projects/000_bazel-hello:hello up-to-date:
  bazel-bin/projects/000_bazel-hello/hello
INFO: Elapsed time: 2.424s, Critical Path: 0.02s
INFO: 1 process: 162 action cache hit, 1 internal.
INFO: Build completed successfully, 1 total action
INFO: Running command line: bazel-bin/projects/000_bazel-hello/hello
Hello, Bazel!
```

- [x] `bazel test //projects/000_bazel-hello:hello_test` — 1 test target passes
```
bazel test //projects/000_bazel-hello:hello_test
INFO: Analyzed target //projects/000_bazel-hello:hello_test (176 packages loaded, 3782 targets configured).
INFO: Found 1 test target...
Target //projects/000_bazel-hello:hello_test up-to-date:
  bazel-bin/projects/000_bazel-hello/test-1774360528/hello_test
INFO: Elapsed time: 24.372s, Critical Path: 23.54s
INFO: 251 processes: 155 internal, 97 darwin-sandbox.
INFO: Build completed successfully, 251 total actions
//projects/000_bazel-hello:hello_test                                    PASSED in 0.3s

Executed 1 out of 1 test: 1 test passes.
```

- [x] `bazel build //projects/000_bazel-hello:hello_lib` — succeeds
```
bazel build //projects/000_bazel-hello:hello_lib
INFO: Analyzed target //projects/000_bazel-hello:hello_lib (0 packages loaded, 0 targets configured).
INFO: Found 1 target...
Target //projects/000_bazel-hello:hello_lib up-to-date:
  bazel-bin/projects/000_bazel-hello/libhello_lib-567196535.rlib
INFO: Elapsed time: 0.160s, Critical Path: 0.05s
INFO: 2 processes: 1 internal, 1 darwin-sandbox.
INFO: Build completed successfully, 2 total actions
```

---

## Exercise 2 — Add a third-party dependency

This is the workflow you will use in every Rust project. Add a crate, wire it into the build, use it in code.

- [x] Add `rand = "0.9"` to `third-party/rust/Cargo.toml`
- [x] Run `CARGO_BAZEL_REPIN=1 bazel build //projects/000_bazel-hello:hello_lib` — Bazel updates `Cargo.lock`, fetches, and compiles the new crate
- [x] Add `"@crates//:rand"` to `deps` in `projects/000_bazel-hello/BUILD`
- [x] In `src/lib.rs`, add `pub fn greet_random() -> String` that picks a random greeting from a fixed list using `rand`
- [x] Write a `#[test]` that calls `greet_random()` and asserts the result is non-empty

**Verify:**
- [x] `bazel test //projects/000_bazel-hello:hello_test` passes
```
bazel test //projects/000_bazel-hello:hello_test
INFO: Analyzed target //projects/000_bazel-hello:hello_test (0 packages loaded, 0 targets configured).
INFO: Found 1 test target...
Target //projects/000_bazel-hello:hello_test up-to-date:
  bazel-bin/projects/000_bazel-hello/test-1774360528/hello_test
INFO: Elapsed time: 0.673s, Critical Path: 0.57s
INFO: 4 processes: 2 internal, 3 darwin-sandbox.
INFO: Build completed successfully, 4 total actions
//projects/000_bazel-hello:hello_test                                    PASSED in 0.3s

Executed 1 out of 1 test: 1 test passes.
```

- [x] `bazel query @crates//:rand` resolves without error
```
bazel query @crates//:rand
@crates//:rand
```
