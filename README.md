# Playground

LLMs are remarkably good at helping you learn, but there's a paradox: the more you rely on them to do your job, the less you actually learn. This repo is my attempt to leverage LLMs to experiment.

**IMPORTANT** Nothing in this repo is meant for production. It's a learning sandbox.

This repo uses [Bazel](https://bazel.build) as the build system. It handles Python and Rust from one place, scales to large monorepos, and is worth learning as some of the biggest companies use it.

## Projects

| Project | Description |
|---------|-------------|
| [000_bazel-hello](projects/000_bazel-hello/) | Verify the Bazel setup |

## Setup

**Install Bazelisk** (the recommended Bazel launcher — picks the right Bazel version automatically):

```
# macOS
brew install bazelisk

# Linux
curl -Lo bazel https://github.com/bazelbuild/bazelisk/releases/latest/download/bazelisk-linux-amd64
chmod +x bazel && sudo mv bazel /usr/local/bin/bazel
```

**Install Docker** (for running local LLM inference — required from project 004 onwards):
```
# https://docs.docker.com/get-started/get-docker/
```

Verify:

```
bazel version
docker --version
```