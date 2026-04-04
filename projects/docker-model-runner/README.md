# Docker Model Runner

Explore [Docker Model Runner](https://docs.docker.com/desktop/features/model-runner/) — Docker Desktop's built-in inference engine — then go deeper by building your own inference server with [llama.cpp](https://github.com/ggerganov/llama.cpp). Understand what Model Runner gives you for free and what it costs to do it yourself.

**Language:** C/C++ (llama.cpp), Go (gateway — reuse from `llm-inference-gateway`)
**Runtime:** Docker Desktop with Model Runner enabled

> **Depends on:** Exercises in this project assume you have a working gateway from [llm-inference-gateway](../llm-inference-gateway/). You'll point that gateway at new backends here.

## Prerequisites

- Docker Desktop 4.40+ with Model Runner enabled
- Go 1.22+ (for gateway modifications)
- C/C++ toolchain (for building llama.cpp — `gcc`/`clang`, `cmake`, `make`)
- A completed gateway from the `llm-inference-gateway` project

### Enable Docker Model Runner

Open Docker Desktop > Settings > Features in Development > enable **Docker Model Runner**. Verify:

```bash
docker model --help
docker model list
```

### Reference

- [Docker Model Runner docs](https://docs.docker.com/desktop/features/model-runner/)
- [OpenAI Chat Completions API reference](https://platform.openai.com/docs/api-reference/chat/completions)
- [Docker `model` CLI reference](https://docs.docker.com/reference/cli/docker/model/)
- [llama.cpp documentation](https://github.com/ggerganov/llama.cpp)
- [llama-server documentation](https://github.com/ggerganov/llama.cpp/tree/master/examples/server)
- [GGUF format spec](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)

---

## Part 0: Docker Model Runner as a Drop-In Backend

### Exercise 0.1 — Pull Models & Explore the API

Pull a small model and explore what Model Runner provides out of the box:

```bash
docker model pull ai/llama3.2:1B-Q8_0
```

Model Runner exposes an OpenAI-compatible API on port `12434`. Explore it — what endpoints are available? How does the response format compare to Ollama's `/api/generate` and `/api/chat`?

**Acceptance criteria:**
- `docker model list` shows the pulled model
- You've hit the `/v1/models` and `/v1/chat/completions` endpoints and understand the response schema
- You can articulate 3 differences between Model Runner's API and Ollama's API (format, fields, streaming format, token counting, etc.)

### Exercise 0.2 — Point Your Gateway at Model Runner

Your gateway from `llm-inference-gateway` talks to Ollama's API. Model Runner speaks OpenAI's API instead. Make your gateway work with Model Runner as a backend.

Think about what needs to change:
- Ollama uses `/api/generate` and `/api/chat` — Model Runner uses `/v1/chat/completions`
- Ollama streams newline-delimited JSON — Model Runner streams SSE (`data: {...}`)
- Request and response field names differ

**Acceptance criteria:**
- Your gateway can proxy requests to Model Runner at `http://localhost:12434`
- Streaming and non-streaming both work through the gateway
- Your existing benchmark script runs against the gateway backed by Model Runner
- Compare the numbers: how does Model Runner's throughput and latency compare to Ollama for the same model size?

### Exercise 0.3 — Container-Native Access

Model Runner provides `model-runner.docker.internal` — a DNS name automatically available inside any Docker container. No port mapping, no extra network config, no sidecar.

Update your gateway's `docker-compose.yml` to use this instead of `host.docker.internal` or hardcoded IPs.

**Acceptance criteria:**
- Gateway container reaches Model Runner via `http://model-runner.docker.internal`
- No `extra_hosts`, `network_mode: host`, or port mapping needed for the Model Runner connection
- `docker compose up -d` starts the gateway and it can serve requests
- Compare this setup to your Ollama docker-compose — how many fewer lines of configuration?

---

## Part 1: Build Your Own Inference Server with llama.cpp

Docker Model Runner uses a bundled inference engine internally. Now build one yourself.

### Exercise 1.1 — Build llama.cpp from Source

Clone [llama.cpp](https://github.com/ggerganov/llama.cpp) and build it. The project uses `cmake`.

Focus on getting `llama-server` built — this is the HTTP server component that exposes an OpenAI-compatible API.

**Acceptance criteria:**
- `llama-server --help` works and shows available flags
- You understand what these build flags do: `-DGGML_CUDA=ON`, `-DGGML_METAL=ON`, `-DGGML_CPU_ALL_VARIANTS=ON` (read the docs, you may not have all the hardware)
- You can explain: what is GGML and how does it relate to llama.cpp?

### Exercise 1.2 — Download a GGUF Model & Serve It

Docker Model Runner pulls models via `docker model pull` from OCI registries. For llama.cpp, you need a raw GGUF file.

Find and download the same model you used with Model Runner (`llama3.2 1B`) in GGUF format. Start `llama-server` with it.

Hints: Hugging Face hosts GGUF files. Look for the quantization that matches what you pulled earlier (Q8_0).

**Acceptance criteria:**
- `llama-server` is running and serving on port 8081
- `curl http://localhost:8081/v1/models` returns a response
- `curl http://localhost:8081/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"llama3.2","messages":[{"role":"user","content":"Hello"}]}'` returns a valid response
- The same curl command format works against both Model Runner (port 12434) and your server (port 8081)

### Exercise 1.3 — Understand the Serving Parameters

`llama-server` has many knobs that affect performance. Experiment with these flags and observe the impact:

- `-c` (context size)
- `-np` (number of parallel slots)
- `-ngl` (number of GPU layers — if you have a GPU)
- `--threads` / `-t`
- `--batch-size` / `-b`

**Acceptance criteria:**
- Run a fixed benchmark (same prompt, 10 requests) under at least 3 different configurations (vary context size, thread count, parallel slots)
- Record tokens/second for each configuration
- You can answer: What is a "slot" in llama-server? What happens when all slots are busy? How does context size affect memory usage and speed?

### Exercise 1.4 — Containerize llama-server

Write a Dockerfile that builds llama.cpp from source and produces an image with `llama-server`. The model GGUF file should be provided at runtime via a volume mount, not baked into the image.

**Acceptance criteria:**
- `docker build -t llama-server .` succeeds
- `docker run -v /path/to/models:/models -p 8081:8081 llama-server -m /models/llama3.2-1B-Q8_0.gguf --host 0.0.0.0 --port 8081` starts serving
- Image builds are reproducible (pinned base image, specific llama.cpp commit)
- You can answer: why mount the model as a volume instead of `COPY` in the Dockerfile?

---

## Part 2: Dual Backend Gateway

### Exercise 2.1 — Backend Abstraction

Refactor your gateway to support multiple backend types behind a common interface. You now have three possible backends:

1. **Ollama** — `/api/chat`, newline-delimited JSON streaming
2. **Docker Model Runner** — `/v1/chat/completions`, SSE streaming
3. **llama-server** — `/v1/chat/completions`, SSE streaming (same API as Model Runner)

Design a backend interface in Go. What's the minimal contract? Think about:
- How do you abstract over different request/response formats?
- Model Runner and llama-server share the same API — how do you handle that cleanly?
- Where does backend selection happen (config, per-request, auto)?

**Acceptance criteria:**
- Gateway supports all three backend types
- Backend is selectable via environment variable (`BACKEND=ollama|model-runner|llama-cpp`)
- All three backends produce the same response format to the client (your gateway normalizes)
- Adding a new OpenAI-compatible backend in the future would require minimal code

### Exercise 2.2 — Docker Compose Full Stack

Write a `docker-compose.yml` that runs:
1. Your containerized llama-server (from Exercise 1.4) with a mounted GGUF model
2. Your gateway, configured to route to llama-server

Then write a second compose override or profile that swaps the backend to Model Runner (using `model-runner.docker.internal`) with no llama-server container needed.

**Acceptance criteria:**
- `docker compose --profile llama up -d` starts gateway + llama-server
- `docker compose --profile model-runner up -d` starts gateway only (uses Model Runner)
- Both profiles serve the same API to clients on the same port
- You can answer: what are the operational trade-offs? (image size, startup time, resource usage, configuration complexity)

### Exercise 2.3 — Head-to-Head Benchmark

Build a benchmark that compares all backends you have available. Same model (or closest equivalent), same prompts, same concurrency levels.

Measure:
- Tokens per second (streaming)
- Time to first token
- p50 / p99 latency under load (1, 5, 10 concurrent requests)
- Memory usage (`docker stats` for containers, `/v1/health` or process stats for native)

**Acceptance criteria:**
- Benchmark script outputs a comparison table across backends and concurrency levels
- Results are reproducible (document exact model, quantization, and hardware)
- You can answer: Does Docker Model Runner add overhead vs running llama-server directly? When would you choose each approach?
