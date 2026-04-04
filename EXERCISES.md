# LLM Inference Systems — Practice Exercises

A structured set of exercises to build intuition for LLM serving infrastructure.
Each section starts simple, then asks you to improve your own code.

**Language:** Python or Go (your choice per exercise).

---

## Part 1: Inference Optimization

### 1.1 — Naive Request Handler

Build an HTTP server that accepts POST requests with a JSON body:

```json
{
  "prompt": "Hello, world",
  "max_tokens": 50
}
```

The server should **simulate** token generation:
- Sleep ~30ms per generated token (simulating GPU compute)
- Stream tokens back one at a time (SSE or chunked response)
- Each "token" can just be a random word from a small vocabulary list

**Goal:** Get a working baseline. Measure: how many requests/sec can it handle?
Benchmark with a script that fires 20 concurrent requests.

---

### 1.2 — Request Batching

Your naive server processes one request at a time per "GPU cycle".
Real inference engines batch multiple requests into a single forward pass.

**Modify your server:**
- Incoming requests go into a queue
- A worker loop runs every 30ms, grabs up to **B** requests from the queue, and generates **one token for each** in a single "step" (one sleep for the whole batch, not per-request)
- Each request tracks how many tokens it has generated and leaves the batch when it hits `max_tokens`

**This is called continuous batching** — requests enter and leave the batch independently.

Questions to answer in comments or a notes file:
1. What happens to latency for a single request when the batch is full vs empty?
2. What happens to throughput as you increase B?
3. What's the trade-off?

---

### 1.3 — Prompt Caching

Many requests share common prefixes (e.g., a system prompt).

**Add a prefix cache:**
- Before processing, check if the prompt starts with a previously seen prefix
- If it does, skip "computing" those tokens (simulate: reduce an artificial `prefill_time = len(prompt) * 2ms` to near-zero for cached prefixes)
- Use an LRU cache with a configurable max size

**Test it:** send 50 requests that all share the same 200-word system prompt but differ in the user message. Compare throughput with and without caching.

---

### 1.4 — KV Cache Memory Pressure

Each in-flight request holds memory (the KV cache). You can't batch infinitely.

**Add memory simulation:**
- Each request "allocates" `prompt_length * 2 + generated_tokens * 2` units of memory
- Set a max memory budget (e.g., 1000 units)
- If admitting a new request to the batch would exceed the budget, it waits in the queue
- When a request finishes, its memory is freed

Questions:
1. What happens when you send many long-prompt requests?
2. What would happen if you preempted (paused) a long request to let short ones through? (You don't need to implement this — just reason about it.)

---

## Part 2: Load Balancing

### 2.1 — Round Robin

Build a reverse proxy that distributes requests across N backend servers (your servers from Part 1).

- Start 3 instances of your inference server on different ports
- The proxy accepts requests and forwards them round-robin
- Return the backend's response to the client

**Keep it simple.** No health checks yet. Just a proxy.

---

### 2.2 — Least Connections

Round-robin is blind to load. A request generating 500 tokens takes 10x longer than one generating 50.

**Change your strategy:**
- Track how many in-flight requests each backend has
- Route to the backend with the fewest in-flight requests
- Decrement when the response completes

**Test:** send a mix of short (max_tokens=10) and long (max_tokens=200) requests.
Compare the p50 and p99 latency vs round-robin.

---

### 2.3 — Health Checks and Failover

Backends crash. Handle it.

- Every 5 seconds, send a GET /health to each backend
- If a backend fails 3 consecutive health checks, remove it from the pool
- If it later passes a health check, re-add it
- While a backend is marked unhealthy, don't route to it

**Test:** start 3 backends, kill one, observe the proxy routing around it. Restart it, observe it rejoining.

---

### 2.4 — Prefix-Aware Routing (LLM-specific)

This is where LLM load balancing gets interesting.

If two requests share the same prompt prefix, routing them to the **same backend** means that backend's prefix cache (from 1.3) gets a hit. Routing them to different backends means both compute the prefix from scratch.

**Implement consistent hashing on the prompt prefix:**
- Hash the first N characters (or first sentence) of the prompt
- Use that hash to pick a backend (consistent hashing ring)
- Fall back to least-connections if that backend is overloaded (e.g., >2x avg load)

Questions:
1. What's the trade-off between cache hit rate and load balance?
2. When would you prefer pure least-connections over prefix-aware routing?

---

## Part 3: Traffic Management

### 3.1 — Rate Limiting

Add rate limiting to your proxy.

- Token bucket algorithm: each client (identified by API key header) gets a bucket
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

---

### 3.3 — Adaptive Concurrency Limiting

Fixed concurrency limits are fragile. Implement an adaptive limit.

- Start with a concurrency limit of 10
- Track request latency over a rolling window (last 100 requests)
- If p99 latency rises above a threshold, reduce the limit by 1 (additive decrease)
- If p99 is healthy, increase by 1 (additive increase)
- Never go below 1 or above 50

This is a simplified version of Netflix's [adaptive concurrency limiter](https://netflixtechblog.medium.com/performance-under-load-3e6fa9a60581).

---

### 3.4 — Request Coalescing

Multiple users often send the exact same prompt (e.g., "Summarize this article" with the same article).

**Implement deduplication:**
- Hash the full request body
- If an identical request is already in-flight, don't send a second one to the backend — wait for the first to complete and return the same response to both clients
- Add a short TTL cache for completed responses (e.g., 5 seconds)

Questions:
1. When is this safe to do? When is it dangerous?
2. How does this interact with non-deterministic generation (temperature > 0)?

---

## Part 4: Putting It Together

### 4.1 — Full Stack

Wire everything together into a single system:

```
Client → Rate Limiter → Priority Queue → Load Balancer (prefix-aware) → Backend Pool
                                                                           ├── Server 1 (batching + caching)
                                                                           ├── Server 2 (batching + caching)
                                                                           └── Server 3 (batching + caching)
```

Write a load test script that:
- Sends 200 requests over 10 seconds
- Mix of priorities, prompt lengths, and max_tokens
- 30% share a common system prompt prefix
- Reports: throughput, p50/p99 latency, cache hit rate, queue depths

---

### 4.2 — Autoscaling Signal

Your system should know when it needs more backends.

- Expose a `/metrics` endpoint on the proxy that reports:
  - Request queue depth
  - Average batch utilization per backend
  - p99 latency over last 60 seconds
  - Memory utilization per backend
- Write a simple controller: if queue depth > 20 for 30 seconds, log "SCALE UP". If all backends are <30% utilized for 60 seconds, log "SCALE DOWN".

You don't need to actually spin up/down servers — just emit the signal.

---

## How to Work Through This

1. **Do exercises in order** — each one builds on the last
2. **Write a benchmark/test for each exercise** — if you can't measure it, you didn't learn it
3. **Write notes** — the "questions to answer" are the kind of thing you'd discuss in an interview
4. **Don't over-engineer** — the first version should be ~50-100 lines. Improve from there.
5. **Compare strategies** — when you implement a new LB strategy, benchmark it against the old one

Good luck. Start with 1.1.
