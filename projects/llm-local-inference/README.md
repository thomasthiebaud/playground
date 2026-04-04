# LLM Local Inference

Build an LLM inference gateway from scratch. Start with [Docker Model Runner](https://docs.docker.com/desktop/features/model-runner/) — Docker Desktop's built-in inference engine — to get running fast, then build your own inference backend with [llama.cpp](https://github.com/ggerganov/llama.cpp) to understand what's under the hood and scale it out.

**Language:** Go
**Build:** Buck2 (`genrule`)
**Runtime:** Docker Desktop with Model Runner enabled, later self-hosted llama.cpp

## Prerequisites

- Docker Desktop 4.40+ with Model Runner enabled
- Go 1.22+
- Buck2 (available as `./buck2` at repo root)
- C/C++ toolchain (for Part 3 — `gcc`/`clang`, `cmake`, `make`)

### Enable Docker Model Runner

Open Docker Desktop > Settings > Features in Development > enable **Docker Model Runner**. Verify:

```bash
docker model --help
docker model list
```

### Reference Docs

Keep these open — you'll need them throughout:

- [Docker Model Runner docs](https://docs.docker.com/desktop/features/model-runner/)
- [Docker `model` CLI reference](https://docs.docker.com/reference/cli/docker/model/)
- [OpenAI Chat Completions API reference](https://platform.openai.com/docs/api-reference/chat/completions)
- [llama.cpp documentation](https://github.com/ggerganov/llama.cpp)
- [llama-server documentation](https://github.com/ggerganov/llama.cpp/tree/master/examples/server)
- [GGUF format spec](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)
- [Docker Compose reference](https://docs.docker.com/reference/compose-file/)
- [Dockerfile reference](https://docs.docker.com/reference/dockerfile/)
- [Buck2 `genrule` docs](https://buck2.build/docs/prelude/globals/#genrule)

---

## Part 0: Setup

### 0.1 — Pull a Model

Pull a small model that runs well on CPU:

```bash
docker model pull ai/llama3.2:1B-Q8_0
```

**Verify:**
- `docker model list` shows `ai/llama3.2:1B-Q8_0`
- `curl http://localhost:12434/v1/models` returns a JSON response listing the model
- `curl http://localhost:12434/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"ai/llama3.2:1B-Q8_0","messages":[{"role":"user","content":"Say hello in one word"}]}'` returns a valid chat completion response

---

### 0.2 — Buck2 Build Target

Write a `BUCK` file in this project directory with a `genrule` that:
- Takes your Go source files as inputs (use `glob`)
- Runs `go build` to produce a binary
- Marks the output as executable

You'll need a `src/main.go` with a basic "hello world" HTTP server to test it.

**Verify:**
```bash
./buck2 build //projects/llm-local-inference:gateway
./buck2 run //projects/llm-local-inference:gateway
```

### OpenAI Chat Completions Cheat Sheet

```bash
# Non-streaming
curl http://localhost:12434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "ai/llama3.2:1B-Q8_0",
    "messages": [{"role": "user", "content": "What is 2+2?"}],
    "stream": false
  }'

# Streaming (returns SSE — Server-Sent Events)
curl -N http://localhost:12434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "ai/llama3.2:1B-Q8_0",
    "messages": [{"role": "user", "content": "What is 2+2?"}],
    "stream": true
  }'

# With system prompt
curl http://localhost:12434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "ai/llama3.2:1B-Q8_0",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "What is 2+2?"}
    ],
    "stream": false
  }'

# List available models
curl http://localhost:12434/v1/models
```

Key response fields: `choices[0].message.content` (non-streaming), `choices[0].delta.content` (streaming), `usage.prompt_tokens`, `usage.completion_tokens`, `usage.total_tokens`.

---

## Part 1: Inference Optimization

### 1.1 — Proxy to Model Runner

Build an HTTP server that accepts POST requests on `/v1/chat/completions` and proxies them to Docker Model Runner at `http://localhost:12434`.

Your server should:
- Forward the request as-is (Model Runner already speaks OpenAI format)
- Stream SSE responses back to the client
- Log the model name, `prompt_tokens`, `completion_tokens`, and `total_tokens` from the response

Update your BUCK target to build this.

**Goal:** Get a working baseline. Write a benchmark script that fires 10 concurrent requests and measures total time, per-request latency, and throughput (requests/sec).

---

### 1.2 — Request Queuing

Model Runner handles concurrency internally, but you want to control admission from your gateway.

**Build a request queue:**
- Incoming requests are placed in a FIFO queue
- A configurable number of workers (start with 1) pull from the queue and forward to Model Runner
- Track and log: queue depth, wait time (time in queue before forwarding), total latency

**Experiment:**
- Vary worker count (1, 2, 4) and benchmark at each level
- Compare throughput and latency vs your 1.1 baseline

Questions to answer:
1. What happens to p99 latency as you increase concurrent workers beyond what the backend can handle?
2. What's the relationship between queue depth and latency?
3. Why would you want your proxy to limit concurrency rather than letting the backend queue grow unbounded?

---

### 1.3 — Prompt Prefix Caching

Many requests share common prefixes (e.g., a system prompt). LLM backends can reuse KV cache for repeated prefixes, skipping re-evaluation of cached tokens.

**Build a prefix-aware cache layer:**
- Maintain a map of `hash(system_prompt) → recent_response_stats`
- Route requests with the same system prompt consistently (this helps the backend's internal KV cache)
- Log `prompt_tokens` and total latency — when the KV cache hits, prompt evaluation should be faster

**Test it:**
1. Send 20 requests all using the same system prompt but different user messages
2. Send 20 requests each with a unique system prompt
3. Compare latency between the two groups

Questions:
1. How significant is the speedup from prefix caching in practice?
2. What happens to the cache when you switch between different system prompts rapidly?

---

### 1.4 — Response Caching

Exact same prompt → exact same answer (when temperature=0).

**Add a response cache:**
- Hash the full request (model + messages + parameters)
- If you've seen this exact request before and `temperature` is 0, return the cached response immediately without hitting the backend
- Use an LRU cache with configurable max entries
- Track and expose: cache hit rate, cache size, estimated time saved

**Test it:** send a burst of 50 identical requests. How does latency compare for cache hits vs misses?

Questions:
1. When is response caching safe vs dangerous?
2. How would you handle cache invalidation when the model is updated?
3. What's the memory cost trade-off?

---

## Part 2: Traffic Management

### 2.1 — Rate Limiting

Add rate limiting to your proxy.

- Token bucket algorithm: each client (identified by an `X-API-Key` header) gets a bucket
- Bucket capacity: 10 requests, refill rate: 2 requests/sec
- Return 429 Too Many Requests when the bucket is empty

Implement the token bucket yourself — don't use a library.

---

### 2.2 — Priority Queues

Not all requests are equal. Add priority support.

- Requests include a `"priority": "high" | "normal" | "low"` field
- The proxy maintains separate queues per priority
- High-priority requests are forwarded first, even if normal/low arrived earlier
- Low-priority requests can be shed (return 503) if total queue depth exceeds a threshold

**Test:** saturate the system with low-priority requests, then send a high-priority one. Measure how long the high-priority request waits.

---

### 2.3 — Adaptive Concurrency Limiting

Fixed concurrency limits are fragile. Implement an adaptive limit.

- Start with a concurrency limit of 5
- Track request latency over a rolling window (last 50 requests)
- If p99 latency rises above a threshold (e.g., 2x your observed baseline), reduce the limit by 1
- If p99 is healthy, increase by 1
- Never go below 1 or above 20

This is a simplified version of Netflix's [adaptive concurrency limiter](https://netflixtechblog.medium.com/performance-under-load-3e6fa9a60581).

**Test:** gradually increase load and observe the limiter adapting. Log the concurrency limit over time.

---

### 2.4 — Request Coalescing

Multiple users often send the exact same prompt.

**Implement deduplication:**
- Hash the full request body
- If an identical request is already in-flight, don't send a second one to the backend — wait for the first to complete and return the same response to both clients
- Add a short TTL cache for completed responses (e.g., 10 seconds)

**Test:** fire 10 identical requests simultaneously. Only 1 should hit the backend.

Questions:
1. When is this safe to do? When is it dangerous?
2. How does this interact with non-deterministic generation (temperature > 0)?

---

## Part 3: Build It Yourself with llama.cpp

Docker Model Runner runs inference for you. Now build your own inference backend to understand what's happening inside.

### 3.1 — Build llama.cpp from Source

Clone [llama.cpp](https://github.com/ggerganov/llama.cpp) and build it. The project uses `cmake`.

Focus on getting `llama-server` built — the HTTP server component that exposes an OpenAI-compatible API.

**Acceptance criteria:**
- `llama-server --help` works and shows available flags
- You understand what these build flags do: `-DGGML_CUDA=ON`, `-DGGML_METAL=ON`, `-DGGML_CPU_ALL_VARIANTS=ON` (read the docs, you may not have all the hardware)
- You can explain: what is GGML and how does it relate to llama.cpp?

---

### 3.2 — Download a GGUF Model & Serve It

Docker Model Runner pulls models via `docker model pull` from OCI registries. For llama.cpp, you need a raw GGUF file.

Find and download the same model you used with Model Runner (`llama3.2 1B`) in GGUF format. Start `llama-server` with it.

Hints: Hugging Face hosts GGUF files. Look for the quantization that matches what you pulled earlier (Q8_0).

**Acceptance criteria:**
- `llama-server` is running and serving on port 8081
- `curl http://localhost:8081/v1/models` returns a response
- `curl http://localhost:8081/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"llama3.2","messages":[{"role":"user","content":"Hello"}]}'` returns a valid response
- The same curl command format works against both Model Runner (port 12434) and your llama-server (port 8081)

---

### 3.3 — Understand the Serving Parameters

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

---

### 3.4 — Containerize llama-server

Write a Dockerfile that builds llama.cpp from source and produces an image with `llama-server`. The model GGUF file should be provided at runtime via a volume mount, not baked into the image.

**Acceptance criteria:**
- `docker build -t llama-server .` succeeds
- `docker run -v /path/to/models:/models -p 8081:8081 llama-server -m /models/llama3.2-1B-Q8_0.gguf --host 0.0.0.0 --port 8081` starts serving
- Image builds are reproducible (pinned base image, specific llama.cpp commit)
- You can answer: why mount the model as a volume instead of `COPY` in the Dockerfile?

---

## Part 4: Load Balancing

Now that you can run your own inference backends, scale out to multiple instances.

### 4.1 — Multi-Backend Docker Setup

Update your `docker-compose.yml` to run 3 llama-server instances on different ports (8081, 8082, 8083). Each needs the same model mounted.

**Verify:** all three instances respond to `/v1/models`.

---

### 4.2 — Round Robin

Build a reverse proxy that distributes requests across your llama-server instances.

- Accept requests on a single port
- Forward to backends in round-robin order
- Stream SSE responses back to the client
- Log which backend handled each request

**Keep it simple.** No health checks yet.

---

### 4.3 — Least Connections

Round-robin is blind to load. A request generating 500 tokens ties up a backend much longer than one generating 50.

**Change your strategy:**
- Track how many in-flight requests each backend has
- Route to the backend with the fewest in-flight requests
- Decrement when the response completes

**Test:** send a mix of short (`max_tokens: 10`) and long (`max_tokens: 200`) requests. Compare p50 and p99 latency vs round-robin.

---

### 4.4 — Health Checks and Failover

Backends crash. Handle it.

- Every 5 seconds, send a GET to each backend's `/v1/models` endpoint
- If a backend fails 3 consecutive health checks, remove it from the pool
- If it later passes a health check, re-add it
- While a backend is marked unhealthy, don't route to it

**Test:** `docker compose stop llama-2`, observe routing around it. `docker compose start llama-2`, observe it rejoining.

---

### 4.5 — Prefix-Aware Routing (LLM-specific)

This is where LLM load balancing diverges from generic load balancing.

If two requests share the same system prompt, routing them to the **same backend** means that backend's KV cache gets a hit. Routing them to different backends means both compute the prefix from scratch.

**Implement consistent hashing on the prompt prefix:**
- Hash the system message content
- Use that hash to pick a backend (consistent hashing ring)
- Fall back to least-connections if the target backend is overloaded (>2x average in-flight count)

**Test:**
- Send 30 requests with system prompt A, 30 with system prompt B
- Compare latency with prefix-aware routing vs round-robin
- You should see measurably lower latency with prefix-aware routing for same-prefix requests

Questions:
1. What's the trade-off between cache hit rate and load balance?
2. When would you prefer pure least-connections over prefix-aware routing?

---

## Part 5: Putting It Together

### 5.1 — Containerize the Gateway

Write a `Dockerfile` for your Go gateway using a multi-stage build:
- Build stage: compile the binary
- Runtime stage: minimal image with just the binary

Add the gateway service to your `docker-compose.yml` so it starts alongside the llama-server backends. The gateway should connect to backends via Docker's internal network (service names), not localhost.

**Verify:** `docker compose up -d` starts everything. Requests to the gateway port get proxied to backends.

---

### 5.2 — Compose Profiles: Model Runner vs Self-Hosted

Add Docker Compose profiles so you can choose your backend:

- `docker compose --profile model-runner up -d` — starts only the gateway, pointing at `model-runner.docker.internal`
- `docker compose --profile self-hosted up -d` — starts the gateway + 3 llama-server instances

Both profiles serve the same API to clients on the same port.

**Questions to answer:** what are the operational trade-offs? (image size, startup time, resource usage, configuration complexity, scaling)

---

### 5.3 — Full Stack

Wire everything together into a single self-hosted system:

```
Client → Rate Limiter → Priority Queue → Load Balancer (prefix-aware) → Backend Pool
                                                                           ├── llama-1
                                                                           ├── llama-2
                                                                           └── llama-3
```

The entire stack runs with `docker compose --profile self-hosted up -d`.

Write a load test script that:
- Sends 50 requests over 30 seconds (adjust to your hardware)
- Mix of priorities, prompt lengths, and max_tokens
- 30% share a common system prompt
- Reports: throughput, p50/p99 latency, cache hit rate, queue depths, per-backend request count

---

### 5.4 — Metrics and Autoscaling Signal

Your system should know when it needs more backends.

- Expose a `/metrics` endpoint on the gateway that reports:
  - Request queue depth
  - In-flight requests per backend
  - p99 latency over last 60 seconds
  - Cache hit rate
- Write a simple controller: if queue depth > 10 for 30 seconds, log "SCALE UP". If all backends have < 1 in-flight request for 60 seconds, log "SCALE DOWN".

You don't need to actually spin up/down containers — just emit the signal. (But if you want to, `docker compose up --scale llama=5` is right there.)

---

### 5.5 — Head-to-Head Benchmark

Compare your self-hosted stack against the Docker Model Runner single-backend setup. Same prompts, same concurrency levels.

Measure:
- Tokens per second
- Time to first token
- p50 / p99 latency under load (1, 5, 10 concurrent requests)
- Memory usage (`docker stats`)

**Acceptance criteria:**
- Benchmark script outputs a comparison table
- Results are reproducible (document exact model, quantization, and hardware)
- You can answer: Does Docker Model Runner add overhead vs running llama-server directly? When does self-hosted with load balancing beat a single Model Runner instance?

---

## How to Work Through This

1. **Do exercises in order** — each one builds on the last
2. **Write a benchmark/test for each exercise** — if you can't measure it, you didn't learn it
3. **Write notes** — the "questions to answer" are the kind of thing you'd discuss in an interview
4. **Don't over-engineer** — the first version should be ~50-100 lines. Improve from there
5. **Compare strategies** — when you implement a new LB strategy, benchmark it against the old one

Good luck. Start with 0.1.
