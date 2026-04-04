# LLM Inference Gateway — Practice Exercises

A structured set of exercises to build intuition for LLM serving infrastructure.
Each section starts simple, then asks you to improve your own code.

**Language:** Python or Go (your choice per exercise).

**Backend:** [Ollama](https://ollama.com/) running a small model, managed via Docker Compose.

**Build:** [Buck2](https://buck2.build/) for building your proxy/gateway code.

## Prerequisites

1. Docker and Docker Compose
2. Buck2 (available via `./buck2` at the repo root)

That's it. Ollama runs in Docker — no local install needed.

## Getting Started

### 1. Start Ollama

```bash
docker compose up -d
```

This starts a single Ollama instance and pulls `qwen2:0.5b` (~300MB, runs on CPU).

### 2. Verify it works

```bash
curl http://localhost:11434/api/generate -d '{"model":"qwen2:0.5b","prompt":"Hi","stream":false}'
```

### 3. Build & run your code

```bash
# From the repo root
./buck2 build //projects/llm-inference-gateway/...
./buck2 run //projects/llm-inference-gateway:gateway
```

### Ollama API Cheat Sheet

```bash
# Non-streaming
curl http://localhost:11434/api/generate -d '{
  "model": "qwen2:0.5b",
  "prompt": "What is 2+2?",
  "stream": false
}'

# Streaming (returns newline-delimited JSON, one token at a time)
curl http://localhost:11434/api/generate -d '{
  "model": "qwen2:0.5b",
  "prompt": "What is 2+2?",
  "stream": true
}'

# Chat format
curl http://localhost:11434/api/chat -d '{
  "model": "qwen2:0.5b",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "What is 2+2?"}
  ],
  "stream": false
}'

# Check loaded models (useful for health checks)
curl http://localhost:11434/api/tags
```

Key response fields: `response` (generated text), `total_duration`, `load_duration`, `prompt_eval_count`, `prompt_eval_duration`, `eval_count`, `eval_duration`. These give you real timing data to work with.

---

## Part 1: Inference Optimization

### 1.1 — Naive Proxy

Build an HTTP server that accepts POST requests and proxies them to Ollama:

```json
{
  "prompt": "Explain what a hash table is in one sentence.",
  "max_tokens": 100
}
```

Your server should:
- Translate the request to Ollama's `/api/generate` format
- Forward it to Ollama and stream the response back to the client
- Log the timing info Ollama returns (`prompt_eval_duration`, `eval_duration`)

**Build & run:**
- Add a build target in `projects/llm-inference-gateway/BUCK` for your proxy
- Build with `./buck2 build //projects/llm-inference-gateway:gateway`
- Run with `./buck2 run //projects/llm-inference-gateway:gateway`
- Ollama is already running via `docker compose up -d`

**Goal:** Get a working baseline. Write a benchmark script that fires 10 concurrent requests and measures total time, per-request latency, and throughput (requests/sec).

**Note:** Ollama serializes requests by default (one at a time). This is your baseline to improve against.

---

### 1.2 — Request Queuing and Batching Awareness

Ollama handles batching internally, but it queues requests sequentially by default.
You'll build a **queue in front of Ollama** that controls admission.

**Build a request queue:**
- Incoming requests are placed in a FIFO queue
- A configurable number of workers (start with 1) pull from the queue and forward to Ollama
- Track and log: queue depth, wait time (time in queue before forwarding), total latency

**Experiment:**
- Edit `docker-compose.yml` to set `OLLAMA_NUM_PARALLEL=2` on the Ollama service, then `docker compose up -d` to apply
- Set your worker count to 2 to match
- Benchmark again: compare throughput vs your 1.1 baseline

Questions to answer:
1. What happens to p99 latency as you increase concurrent workers beyond what Ollama can handle?
2. What's the relationship between queue depth and latency?
3. Why would you want your proxy to limit concurrency rather than letting Ollama's queue grow unbounded?

---

### 1.3 — Prompt Caching (KV Cache Reuse)

Many requests share common prefixes (e.g., a system prompt). Ollama supports the `keep_alive` parameter and reuses KV cache for repeated prefixes on the same model.

**Build a prefix-aware cache layer:**
- Maintain a map of `hash(system_prompt) → recent_response_stats`
- For requests with the same system prompt prefix, send them to Ollama using the `/api/chat` endpoint with a consistent system message
- Log `prompt_eval_count` and `prompt_eval_duration` — when the KV cache hits, `prompt_eval_count` should drop (Ollama skips re-evaluating cached prompt tokens)

**Test it:**
1. Send 20 requests all using the same system prompt but different user messages
2. Send 20 requests each with a unique system prompt
3. Compare `prompt_eval_duration` between the two groups

Questions:
1. How significant is the speedup from prefix caching in practice?
2. What happens to the cache when you switch between different system prompts rapidly?

---

### 1.4 — Response Caching

Exact same prompt → exact same answer (when temperature=0).

**Add a response cache:**
- Hash the full request (model + prompt + parameters)
- If you've seen this exact request before and `temperature` is 0, return the cached response immediately without hitting Ollama
- Use an LRU cache with configurable max entries
- Track and expose: cache hit rate, cache size, estimated time saved

**Test it:** send a burst of 50 identical requests. How does latency compare for cache hits vs misses?

Questions:
1. When is response caching safe vs dangerous?
2. How would you handle cache invalidation when the model is updated?
3. What's the memory cost trade-off?

---

## Part 2: Load Balancing

For this section, you'll scale up to **multiple Ollama instances** using Docker Compose.

**Setup:** Update `docker-compose.yml` to run 3 Ollama instances:

```yaml
services:
  ollama-1:
    image: ollama/ollama:latest
    ports:
      - "11434:11434"
    volumes:
      - ollama1-data:/root/.ollama

  ollama-2:
    image: ollama/ollama:latest
    ports:
      - "11435:11434"
    volumes:
      - ollama2-data:/root/.ollama

  ollama-3:
    image: ollama/ollama:latest
    ports:
      - "11436:11434"
    volumes:
      - ollama3-data:/root/.ollama
```

Then `docker compose up -d` and pull the model on each instance:
```bash
docker compose exec ollama-1 ollama pull qwen2:0.5b
docker compose exec ollama-2 ollama pull qwen2:0.5b
docker compose exec ollama-3 ollama pull qwen2:0.5b
```

### 2.1 — Round Robin

Build a reverse proxy that distributes requests across your backend instances.

- Accept requests on a single port
- Forward to backends in round-robin order
- Return the backend's streamed response to the client
- Log which backend handled each request

**Keep it simple.** No health checks yet.

---

### 2.2 — Least Connections

Round-robin is blind to load. A request generating 500 tokens ties up a backend much longer than one generating 50.

**Change your strategy:**
- Track how many in-flight requests each backend has
- Route to the backend with the fewest in-flight requests
- Decrement when the response completes

**Test:** send a mix of short (`max_tokens=10`) and long (`max_tokens=200`) requests.
Compare the p50 and p99 latency vs round-robin. Use the real Ollama timing data.

---

### 2.3 — Health Checks and Failover

Backends crash. Handle it.

- Every 5 seconds, send a GET to each backend's Ollama (`/api/tags` is a good health endpoint)
- If a backend fails 3 consecutive health checks, remove it from the pool
- If it later passes a health check, re-add it
- While a backend is marked unhealthy, don't route to it

**Test:** `docker compose stop ollama-2`, observe routing around it. `docker compose start ollama-2`, observe it rejoining.

---

### 2.4 — Prefix-Aware Routing (LLM-specific)

This is where LLM load balancing diverges from generic load balancing.

If two requests share the same system prompt, routing them to the **same backend** means that backend's KV cache gets a hit (from 1.3). Routing them to different backends means both compute the prefix from scratch.

**Implement consistent hashing on the prompt prefix:**
- Hash the system prompt (or first N characters of the prompt)
- Use that hash to pick a backend (consistent hashing ring)
- Fall back to least-connections if the target backend is overloaded (>2x average in-flight count)

**Test with real Ollama data:**
- Send 30 requests with system prompt A, 30 with system prompt B
- Compare `prompt_eval_duration` with prefix-aware routing vs round-robin
- You should see measurably lower eval times with prefix-aware routing

Questions:
1. What's the trade-off between cache hit rate and load balance?
2. When would you prefer pure least-connections over prefix-aware routing?

---

## Part 3: Traffic Management

### 3.1 — Rate Limiting

Add rate limiting to your proxy.

- Token bucket algorithm: each client (identified by an `X-API-Key` header) gets a bucket
- Bucket capacity: 10 requests, refill rate: 2 requests/sec
- Return 429 Too Many Requests when the bucket is empty

Implement the token bucket yourself — don't use a library.

---

### 3.2 — Priority Queues

Not all requests are equal. Add priority support.

- Requests include a `"priority": "high" | "normal" | "low"` field
- The proxy maintains separate queues per priority
- High-priority requests are forwarded first, even if normal/low arrived earlier
- Low-priority requests can be shed (return 503) if total queue depth exceeds a threshold

**Test:** saturate the system with low-priority requests, then send a high-priority one. Measure how long the high-priority request waits.

---

### 3.3 — Adaptive Concurrency Limiting

Fixed concurrency limits are fragile. Implement an adaptive limit.

- Start with a concurrency limit of 5
- Track request latency over a rolling window (last 50 requests)
- If p99 latency rises above a threshold (e.g., 2x your observed baseline), reduce the limit by 1
- If p99 is healthy, increase by 1
- Never go below 1 or above 20

This is a simplified version of Netflix's [adaptive concurrency limiter](https://netflixtechblog.medium.com/performance-under-load-3e6fa9a60581).

**Test:** gradually increase load and observe the limiter adapting. Log the concurrency limit over time.

---

### 3.4 — Request Coalescing

Multiple users often send the exact same prompt.

**Implement deduplication:**
- Hash the full request body
- If an identical request is already in-flight, don't send a second one to Ollama — wait for the first to complete and return the same response to both clients
- Add a short TTL cache for completed responses (e.g., 10 seconds)

**Test:** fire 10 identical requests simultaneously. Only 1 should hit Ollama.

Questions:
1. When is this safe to do? When is it dangerous?
2. How does this interact with non-deterministic generation (temperature > 0)?

---

## Part 4: Putting It Together

### 4.1 — Full Stack

Wire everything together into a single system:

```
Client → Rate Limiter → Priority Queue → Load Balancer (prefix-aware) → Backend Pool
                                                                           ├── Ollama :11434
                                                                           ├── Ollama :11435
                                                                           └── Ollama :11436
```

All backends are managed by `docker-compose.yml`. Your gateway is built with Buck2.

**Containerize your gateway** — add a `Dockerfile` that builds the gateway binary and runs it. Add it to `docker-compose.yml` so the full stack starts with one command:

```bash
docker compose up -d
```

Write a load test script that:
- Sends 50 requests over 30 seconds (real inference is slower — adjust to your hardware)
- Mix of priorities, prompt lengths, and max_tokens
- 30% share a common system prompt
- Reports: throughput, p50/p99 latency, cache hit rate, queue depths, per-backend request count

---

### 4.2 — Metrics and Autoscaling Signal

Your system should know when it needs more backends.

- Expose a `/metrics` endpoint on the proxy that reports:
  - Request queue depth
  - In-flight requests per backend
  - p99 latency over last 60 seconds
  - Cache hit rate
  - Per-backend `prompt_eval_duration` average (indicates KV cache effectiveness)
- Write a simple controller: if queue depth > 10 for 30 seconds, log "SCALE UP". If all backends have < 1 in-flight request for 60 seconds, log "SCALE DOWN".

You don't need to actually spin up/down containers — just emit the signal. (But if you want to, `docker compose up --scale ollama=5` is right there.)

---

## How to Work Through This

1. **Do exercises in order** — each one builds on the last
2. **Write a benchmark/test for each exercise** — if you can't measure it, you didn't learn it
3. **Write notes** — the "questions to answer" are the kind of thing you'd discuss in an interview
4. **Don't over-engineer** — the first version should be ~50-100 lines. Improve from there
5. **Compare strategies** — when you implement a new LB strategy, benchmark it against the old one
6. **Read Ollama's timing data** — the real numbers will teach you more than any blog post

### Workflow

```bash
# Start backends
docker compose up -d

# Build & run your gateway
./buck2 build //projects/llm-inference-gateway:gateway
./buck2 run //projects/llm-inference-gateway:gateway

# Run benchmarks
./buck2 run //projects/llm-inference-gateway:bench

# When you reach Part 4, everything runs in Docker
docker compose up -d --build
```

Good luck. Start with 1.1.
