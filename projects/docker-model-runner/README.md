# Docker Model Runner

Build an LLM-powered application using [Docker Model Runner](https://docs.docker.com/desktop/features/model-runner/) — Docker Desktop's built-in inference engine. Start by using the OpenAI-compatible API it provides, then reimplement the inference layer yourself using [llama.cpp](https://github.com/ggerganov/llama.cpp) to understand what happens under the hood.

**Language:** Go
**Build:** Buck2 (`genrule`)
**Runtime:** Docker Desktop with Model Runner enabled

## Prerequisites

- Docker Desktop 4.40+ with Model Runner enabled
- Go 1.22+
- Buck2 (available as `./buck2` at repo root)

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
- [GGUF format spec](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)
- [Buck2 `genrule` docs](https://buck2.build/docs/prelude/globals/#genrule)

---

## Part 0: Setup & First Inference

### Exercise 0.1 — Pull a Model

Pull a small model that runs well on CPU:

```bash
docker model pull ai/llama3.2:1B-Q8_0
```

Verify it's available and that the API is reachable.

**Acceptance criteria:**
- `docker model list` shows `ai/llama3.2:1B-Q8_0`
- `curl http://localhost:12434/v1/models` returns a JSON response listing the model
- `curl http://localhost:12434/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"ai/llama3.2:1B-Q8_0","messages":[{"role":"user","content":"Say hello in one word"}]}'` returns a valid chat completion response

### Exercise 0.2 — Buck2 Build Target

Create a `BUCK` file with a `genrule` that compiles a Go binary. Create a minimal Go HTTP server in `src/main.go` that listens on port 8080 and responds to `GET /health` with `{"status": "ok"}`.

**Acceptance criteria:**
- `cd <repo-root> && ./buck2 build //projects/docker-model-runner:docker-model-runner` succeeds
- Running the built binary and hitting `curl http://localhost:8080/health` returns `{"status": "ok"}`

### Exercise 0.3 — Proxy to Model Runner

Extend your server to accept `POST /v1/chat/completions` requests in OpenAI format and forward them to Docker Model Runner at `http://localhost:12434`.

Stream the response back to the client. Log the model, token counts, and total duration from the response.

**Acceptance criteria:**
- `curl -N http://localhost:8080/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"ai/llama3.2:1B-Q8_0","stream":true,"messages":[{"role":"user","content":"What is Docker?"}]}'` streams back SSE chunks
- Non-streaming requests (`"stream": false`) also work and return a complete JSON response
- Server logs show: model name, `prompt_tokens`, `completion_tokens`, and `total_tokens` from the response

---

## Part 1: Understanding the OpenAI API

### Exercise 1.1 — Conversation Memory

Implement a `/v1/conversations` endpoint that maintains conversation history server-side.

Design the API yourself. Think about:
- How does a client create a new conversation?
- How does a client send a message to an existing conversation?
- How is the full message history sent to the model on each turn?
- What happens when the conversation gets too long for the model's context window?

**Acceptance criteria:**
- A client can create a conversation, send multiple messages, and receive responses that are aware of prior turns
- Verify with a multi-turn interaction: ask the model your name, tell it your name, then ask again — it should remember
- The conversation history is stored in memory (no persistence needed)

### Exercise 1.2 — System Prompts & Temperature

Add support for per-conversation configuration:
- A `system` prompt set at conversation creation time
- A `temperature` parameter (0.0–2.0)

Experiment with how these affect output.

**Acceptance criteria:**
- Create two conversations: one with a system prompt "You are a pirate" and one with "You are a poet". Send the same user message to both — responses should differ in style
- Send the same prompt 5 times with `temperature: 0` — responses should be nearly identical
- Send the same prompt 5 times with `temperature: 1.5` — responses should vary significantly
- Document your observations about temperature's effect on output quality and consistency

### Exercise 1.3 — Streaming vs Non-Streaming Performance

Build a benchmark script (or Go test) that compares streaming vs non-streaming for the same prompt. Measure:
- Time to first token (streaming only)
- Total completion time
- Token throughput (tokens/second)

Run with at least 3 different prompt lengths (short, medium, long).

**Acceptance criteria:**
- Benchmark outputs a table comparing streaming vs non-streaming across prompt lengths
- Time-to-first-token is measured and reported for streaming requests
- You can answer: Is there a throughput difference between streaming and non-streaming? Why or why not?

---

## Part 2: Multi-Model Routing

### Exercise 2.1 — Pull Multiple Models

Pull a second, larger model:

```bash
docker model pull ai/llama3.2:3B-Q4_K_M
```

Verify both models are available via the API.

**Acceptance criteria:**
- `docker model list` shows both models
- `curl http://localhost:12434/v1/models` lists both
- You can send a chat completion to each model individually and get responses

### Exercise 2.2 — Model Router

Add a routing layer to your proxy. Requests include a `model` field — route to the correct model. If the requested model isn't available, return a `404` with a helpful error listing available models.

Add a `GET /v1/models` endpoint to your proxy that queries Model Runner and returns the available models.

**Acceptance criteria:**
- Requests specifying `ai/llama3.2:1B-Q8_0` get routed to the 1B model
- Requests specifying `ai/llama3.2:3B-Q4_K_M` get routed to the 3B model
- Requesting a non-existent model returns `404` with available model names
- `GET /v1/models` on your proxy returns the list from Model Runner

### Exercise 2.3 — Complexity-Based Routing

Instead of requiring clients to pick a model, implement automatic routing based on prompt complexity. Design a heuristic — consider:
- Prompt length (token count estimate)
- Presence of keywords suggesting complex reasoning
- Conversation history length

Route simple queries to the small/fast model and complex queries to the larger model.

**Acceptance criteria:**
- "What is 2+2?" routes to the 1B model
- A long prompt with multiple constraints routes to the 3B model
- Log which model was selected and why for each request
- Benchmark: compare latency of auto-routed requests vs always using the 3B model. For simple prompts, the small model should be noticeably faster

---

## Part 3: Containerize & Compose

### Exercise 3.1 — Dockerfile

Write a multi-stage Dockerfile for your Go gateway:
- **Build stage:** compile the Go binary
- **Runtime stage:** minimal image (e.g., `scratch` or `distroless`) with just the binary

**Acceptance criteria:**
- `docker build -t model-runner-gateway .` succeeds (run from project directory)
- `docker run --rm model-runner-gateway /gateway --help` or similar shows the binary runs
- Final image size is under 20 MB

### Exercise 3.2 — Docker Compose with Model Runner

Write a `docker-compose.yml` that runs your gateway. The gateway should reach Docker Model Runner via `model-runner.docker.internal` (the built-in DNS name available to all containers when Model Runner is enabled).

**Acceptance criteria:**
- `docker compose up -d` starts the gateway
- `curl http://localhost:8080/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"ai/llama3.2:1B-Q8_0","messages":[{"role":"user","content":"Hello"}]}'` returns a response
- The gateway connects to Model Runner without any extra network configuration or port mapping — just `http://model-runner.docker.internal`
- `docker compose logs gateway` shows requests being proxied successfully

### Exercise 3.3 — Health Check & Readiness

Add a health check to your docker-compose service that verifies the gateway can reach Model Runner. The gateway should not accept traffic until at least one model is available.

**Acceptance criteria:**
- `docker compose ps` shows the gateway as `healthy`
- If Model Runner is unreachable, the health check fails and Docker reports `unhealthy`
- The gateway's `/health` endpoint returns model availability info (e.g., `{"status": "ok", "models": 2}`)

---

## Part 4: Build It Yourself with llama.cpp

Now that you understand the API contract from the consumer side, reimplement the inference layer yourself.

### Exercise 4.1 — Build llama.cpp

Clone and build [llama.cpp](https://github.com/ggerganov/llama.cpp) from source. Get the `llama-server` binary running with a GGUF model file.

**Acceptance criteria:**
- `llama-server` starts and serves on a port you choose (e.g., 8081)
- It exposes an OpenAI-compatible `/v1/chat/completions` endpoint
- The same `curl` command you used against Docker Model Runner works against your `llama-server` instance
- Compare response quality and speed between Docker Model Runner and your `llama-server` for the same model and prompt

### Exercise 4.2 — Dual Backend

Update your gateway to support both backends: Docker Model Runner and your self-hosted llama.cpp server. Add configuration to specify which backend to use (environment variable or request header).

**Acceptance criteria:**
- Gateway can route to Docker Model Runner (`model-runner.docker.internal`) or llama.cpp (`localhost:8081`)
- A request with `X-Backend: model-runner` goes to Docker Model Runner
- A request with `X-Backend: llama-cpp` goes to llama.cpp
- Default backend is configurable via environment variable
- Both backends return responses in the same OpenAI-compatible format — the client doesn't need to know which backend served the request

### Exercise 4.3 — Comparative Benchmarks

Build a comprehensive benchmark comparing both backends. Measure for each:
- Tokens per second
- Time to first token (streaming)
- Memory usage (check Docker stats / process stats)
- Behavior under concurrent load (5, 10, 20 simultaneous requests)

**Acceptance criteria:**
- Benchmark script outputs a comparison table for both backends
- Results include p50, p95, and p99 latency at each concurrency level
- Memory usage is captured before and during load
- You can answer: What are the trade-offs between using Docker Model Runner vs self-hosted llama.cpp? When would you choose each?
