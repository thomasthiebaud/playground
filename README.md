# playground

A monorepo for hands-on systems engineering experiments. Each project is self-contained under `projects/`.

## Projects

| Project | Description |
|---------|-------------|
| [llm-local-inference](projects/llm-local-inference/) | LLM inference gateway — start with Docker Model Runner, then build your own backend with llama.cpp |

## Future Work

| Item | Notes |
|------|-------|
| Buck2 Go toolchain | Set up `go_binary`/`go_library` rules so projects can use native Go targets instead of `genrule` shelling out to `go build` |

## Setup

This project uses [DotSlash](https://dotslash-cli.com) to manage **Buck2**. This ensures everyone uses the exact same version without manual installation or compilation errors.

### 1. Install Rust & Cargo

Install Rust (which includes Cargo) via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then follow the on-screen instructions and restart your shell (or run `source $HOME/.cargo/env`) to add Cargo to your `$PATH`.

### 2. Install DotSlash

Ensure you have the DotSlash fetcher installed and in your `$PATH`:

```bash
cargo install dotslash
```

### 3. Verify Buck2

The `buck2` file in this repository is a DotSlash configuration. It is already committed as executable, so you can run it directly:

```bash
./buck2 --version
```

## Common Commands

| Command                        | Description     |
|--------------------------------|-----------------|
| `./buck2 build //...`          | Build everything |
| `./buck2 run //path/to:target` | Run a target    |
| `./buck2 test //...`           | Test everything |
