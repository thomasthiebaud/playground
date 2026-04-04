# LLM Inference — Zero to Hero

A structured learning path for LLM inference systems engineering. Covers the full stack from transformer mechanics to fleet orchestration — the skills needed to build and operate inference infrastructure at scale.

**Language:** Rust (gateway), C/C++ (llama.cpp)
**Build:** Buck2, Cargo
**Runtime:** Docker Desktop with Model Runner, self-hosted llama.cpp

## Prerequisites

- Docker Desktop 4.40+ with Model Runner enabled
- Rust 1.75+ (install via [rustup](https://rustup.rs))
- C/C++ toolchain (`gcc`/`clang`, `cmake`, `make`)
- Buck2 (available as `./buck2` at repo root)
- Python 3.11+ (for benchmarking scripts and vLLM in Part 4)

### Enable Docker Model Runner

Open Docker Desktop > Settings > Features in Development > enable **Docker Model Runner**. Verify:

```bash
docker model --help
docker model list
```

### Reference Docs

- [Docker Model Runner docs](https://docs.docker.com/desktop/features/model-runner/)
- [OpenAI Chat Completions API reference](https://platform.openai.com/docs/api-reference/chat/completions)
- [llama.cpp documentation](https://github.com/ggerganov/llama.cpp)
- [llama-server documentation](https://github.com/ggerganov/llama.cpp/tree/master/examples/server)
- [GGUF format spec](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)
- [vLLM documentation](https://docs.vllm.ai/)
- [Efficient Memory Management for Large Language Model Serving with PagedAttention](https://arxiv.org/abs/2309.06180)
- [Orca: A Distributed Serving System for Transformer-Based Generative Models](https://www.usenix.org/conference/osdi22/presentation/yu)
- [Buck2 `genrule` docs](https://buck2.build/docs/prelude/globals/#genrule)

---

## Part 0: How LLM Inference Works

Before writing any infrastructure code, understand what you're optimizing for.

### 0.1 — Transformer Inference Phases

Read and understand the two distinct phases of autoregressive LLM inference:

1. **Prefill** (prompt processing): all input tokens are processed in parallel to build the KV cache
2. **Decode** (token generation): tokens are generated one at a time, each attending to the full KV cache

**Reading list:**
- [The Illustrated Transformer](https://jalammar.github.io/illustrated-transformer/) — visual walkthrough of attention, the building block of everything below
- [LLM Inference](https://www.databricks.com/blog/llm-inference-performance-engineering-best-practices) — Databricks deep-dive on prefill vs decode, memory-bandwidth bottleneck, KV cache sizing
- [Transformer Inference Arithmetic](https://kipp.ly/transformer-inference-arithmetic/) — the math behind why decode is memory-bound: how to calculate memory bandwidth requirements from model dimensions
- [A Survey on Efficient Inference for Large Language Models](https://arxiv.org/abs/2404.14294) — sections 2–3 cover the inference pipeline and bottleneck analysis (skim the rest for later)

**Acceptance criteria — you can answer:**
- Why is prefill compute-bound and decode memory-bandwidth-bound?
- What is the KV cache? How does its memory grow with sequence length and batch size?
- What determines tokens-per-second during decode? (hint: it's not FLOPs)
- Why does a longer context window cost more memory but not proportionally more compute during decode?
- What is the difference between `prompt_tokens` and `completion_tokens` in terms of computational cost?

---

### 0.2 — Quantization

Pull two versions of the same model:

```bash
docker model pull ai/llama3.2:1B-Q4_K_M
docker model pull ai/llama3.2:1B-Q8_0
```

Send the same prompts to both. Compare output quality, speed, and memory usage.

**Reading list:**
- [GGUF format spec — quantization types](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md) — what Q4_K_M, Q8_0, etc. actually mean
- [Introduction to Quantization](https://mlabonne.github.io/blog/posts/Introduction_to_Weight_Quantization.html) — visual guide to how weights are mapped to lower precision
- [Which GGUF is right for me?](https://www.reddit.com/r/LocalLLaMA/wiki/index/#wiki_which_gguf_quantization_is_right_for_me.3F) — practical guidance on quality/speed trade-offs for common quant levels

**Acceptance criteria:**
- You can explain what quantization does (reduce weight precision from FP16 → INT8/INT4)
- You've measured tokens/sec for Q4_K_M vs Q8_0 on the same hardware
- You can articulate the quality/speed/memory trade-off
- You understand what "K-quant" means in GGUF quantization names
- You can answer: when would you choose Q4 over Q8? When would neither be acceptable?

---

### 0.3 — Benchmarking Fundamentals

Before building anything, establish your measurement toolkit. Write a Python script that:
- Sends requests to `http://localhost:12434/v1/chat/completions`
- Measures: time to first token (TTFT), time per output token (TPOT), end-to-end latency, tokens/sec
- Supports both streaming and non-streaming
- Runs N concurrent requests and reports p50, p95, p99

You'll reuse this throughout every exercise.

**Acceptance criteria:**
- Script can benchmark any OpenAI-compatible endpoint
- Outputs a clean table with TTFT, TPOT, throughput, and latency percentiles
- You understand why TTFT and TPOT are the two metrics that matter most for user experience
- You can answer: why does TTFT increase with prompt length? Why does TPOT stay roughly constant regardless of prompt length?

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

# List available models
curl http://localhost:12434/v1/models
```

Key response fields: `choices[0].message.content` (non-streaming), `choices[0].delta.content` (streaming), `usage.prompt_tokens`, `usage.completion_tokens`, `usage.total_tokens`.

---

## Part 1: Build and Understand a Single Inference Server

### 1.1 — Build llama.cpp from Source

Clone [llama.cpp](https://github.com/ggerganov/llama.cpp) and build it. Focus on `llama-server`.

**Acceptance criteria:**
- `llama-server --help` works
- You understand these build flags: `-DGGML_CUDA=ON`, `-DGGML_METAL=ON`, `-DGGML_CPU_ALL_VARIANTS=ON`
- You can explain: what is GGML? How does it relate to llama.cpp? What computation backends does it support?

---

### 1.2 — Serve a Model

Download the same model you used with Model Runner (Llama 3.2 1B, Q8_0 quantization) as a GGUF file from Hugging Face. Start `llama-server` with it.

**Acceptance criteria:**
- `llama-server` running on port 8081
- Same curl commands work against both Model Runner (:12434) and your server (:8081)
- Run your benchmark script from 0.3 against both — compare numbers

---

### 1.3 — Slots and Continuous Batching

`llama-server` uses **slots** — fixed context windows that requests are assigned to. This is its batching mechanism.

**Reading list:**
- [How continuous batching enables 23x throughput in LLM inference](https://www.anyscale.com/blog/continuous-batching-llm-inference) — clear explanation with diagrams of static vs continuous batching
- [Orca: A Distributed Serving System for Transformer-Based Generative Models](https://www.usenix.org/conference/osdi22/presentation/yu) — the paper that introduced iteration-level scheduling

Experiment with these flags:

- `-c` (total context size across all slots)
- `-np` (number of parallel slots)
- `--batch-size` / `-b` (tokens processed per batch during prefill)
- `--threads` / `-t`

**Acceptance criteria:**
- Benchmark with `-np 1` vs `-np 2` vs `-np 4` at 10 concurrent requests each
- You can answer:
  - What is a "slot"? What happens when all slots are busy?
  - How does `-c` interact with `-np`? (hint: context is divided among slots)
  - What is continuous batching and why does it matter? How is it different from static batching?
  - Read the Orca paper (linked above) — what problem does iteration-level scheduling solve?

---

### 1.4 — KV Cache Mechanics

Observe KV cache behavior in llama-server by watching its logs and `/health` endpoint.

**Reading list:**
- [How prompt caching works — Paged Attention and Automatic Prefix Caching](https://sankalp.bearblog.dev/how-prompt-caching-works/) — walks through how prefix caching is implemented at the KV cache level
- [Prompt Caching with Anthropic](https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching) — how a production API exposes prefix caching to users (the feature you'd be building)
- [Dissecting Batching Effects in GPT Inference](https://le.qun.ch/en/blog/2023/05/13/transformer-batching/) — concrete numbers on how KV cache memory scales with batch size and sequence length

**Experiments:**
1. Send 20 requests with the same system prompt, different user messages. Note prompt processing speed.
2. Send 20 requests each with unique system prompts. Compare.
3. Use `--cache-type-k q8_0` and `--cache-type-v q8_0` flags — measure memory and quality impact.

**Acceptance criteria:**
- You can explain how prefix caching works at the KV cache level
- You understand why quantizing the KV cache saves memory (and how much, approximately)
- You can answer: if a model has 32 layers, 32 attention heads, and head dimension 64, how much KV cache memory does a single token consume in FP16? In Q8_0?

---

### 1.5 — Structured Output (Constrained Decoding)

llama-server supports grammar-constrained generation via the `response_format` field (JSON mode) and GBNF grammars.

**Reading list:**
- [Efficient Guided Generation for Large Language Models](https://arxiv.org/abs/2307.09702) — the Outlines paper: how to constrain generation to a grammar without affecting quality
- [llama.cpp GBNF grammar guide](https://github.com/ggerganov/llama.cpp/blob/master/grammars/README.md) — syntax and examples for writing grammars
- [Structured Outputs — OpenAI](https://platform.openai.com/docs/guides/structured-outputs) — how a production API exposes constrained decoding (the feature you'd be building)

**Experiments:**
1. Send a request with `"response_format": {"type": "json_object"}` — observe the output
2. Write a GBNF grammar that constrains output to a specific JSON schema (e.g., `{"name": string, "age": number}`)
3. Benchmark: how does constrained decoding affect tokens/sec vs unconstrained?

**Acceptance criteria:**
- You can generate valid JSON reliably using grammar-constrained decoding
- You understand how constrained decoding works (hint: it masks logits for invalid next tokens at each step)
- You can answer: why doesn't constrained decoding reduce output quality? What's the performance cost?
- You know what GBNF is and how it relates to context-free grammars

---

### 1.6 — Containerize llama-server

Write a Dockerfile that builds llama.cpp from source. Model files should be mounted at runtime, not baked in.

**Acceptance criteria:**
- `docker build -t llama-server .` succeeds
- `docker run -v /path/to/models:/models -p 8081:8081 llama-server -m /models/<model>.gguf --host 0.0.0.0 --port 8081` serves requests
- Pinned base image and llama.cpp commit for reproducibility
- You can answer: why volume-mount models instead of COPY? (hint: image size, model versioning, multi-model serving)

---

## Part 2: The Gateway

Build an inference gateway in Rust. This is the routing and traffic management layer that sits between clients and backends.

### 2.1 — Proxy with Streaming

Build an HTTP server in Rust that:
- Accepts `POST /v1/chat/completions` in OpenAI format
- Forwards to Docker Model Runner at `http://localhost:12434`
- Streams SSE responses back to the client
- Logs model, token counts, and latency

Use `tokio`, `hyper` or `axum`, and `reqwest`. Set up a Cargo workspace in `src/`.

**Acceptance criteria:**
- Streaming and non-streaming requests both work through your proxy
- Your benchmark script from 0.3 works against the proxy
- Proxy adds < 5ms overhead vs hitting Model Runner directly

---

### 2.2 — Request Queuing & Admission Control

**Reading list:**
- [Performance Under Load — Netflix](https://netflixtechblog.medium.com/performance-under-load-3e6fa9a60581) — adaptive concurrency limiting in production (you'll implement a version of this in 2.7)
- [Little's Law](https://en.wikipedia.org/wiki/Little%27s_law) — the fundamental relationship between throughput, concurrency, and latency: L = λW. Useful for reasoning about queue sizing

Add a bounded queue in front of the backend.

- FIFO queue with configurable max depth
- Configurable worker count pulling from the queue
- Track: queue depth, wait time, total latency
- Return 503 when queue is full (backpressure)

**Experiment:** vary worker count (1, 2, 4) and benchmark.

**Acceptance criteria:**
- You can answer:
  - Why limit concurrency at the proxy instead of letting the backend queue grow?
  - What happens to tail latency as queue depth grows?
  - How would you choose the right queue depth and worker count for production?

---

### 2.3 — Response Caching

Add an LRU response cache:
- Key: hash of (model + messages + temperature + other deterministic params)
- Only cache when temperature=0
- Configurable max entries
- Track hit rate and estimated time saved

**Test:** 50 identical requests — only 1 should hit the backend.

---

### 2.4 — Rate Limiting

Implement token bucket rate limiting per client (`X-API-Key` header):
- Bucket capacity: 10 requests, refill: 2/sec
- Return 429 when empty

Implement from scratch — no library.

---

### 2.5 — Priority Queues

Replace FIFO with priority queuing:
- `"priority": "high" | "normal" | "low"` in the request
- High goes first, low gets shed (503) when queue exceeds threshold

**Test:** saturate with low-priority, send high-priority — measure wait time.

---

### 2.6 — Request Coalescing

Deduplicate identical in-flight requests:
- Hash the request body
- If same request is already in-flight, wait for it instead of sending a duplicate
- Short TTL cache (10s) for completed responses

**Test:** 10 identical concurrent requests — only 1 hits the backend.

Questions:
1. When is coalescing safe? When is it dangerous?
2. How does temperature > 0 interact with this?

---

## Part 3: Multi-Backend Routing

Scale to multiple inference servers and route intelligently.

### 3.1 — Multi-Backend Setup

Update `docker-compose.yml` to run 3 llama-server instances (ports 8081–8083), each with the same model mounted.

**Verify:** all three respond to `/v1/models`.

---

### 3.2 — Round Robin

Distribute requests across backends in round-robin order. Stream responses back. Log which backend handled each request.

---

### 3.3 — Least Connections

Track in-flight requests per backend. Route to the one with the fewest.

**Test:** mix of short and long requests. Compare p50/p99 vs round-robin.

---

### 3.4 — Health Checks & Failover

- Poll `/v1/models` every 5 seconds
- Remove after 3 consecutive failures, re-add when healthy
- Never route to unhealthy backends

**Test:** stop a backend, observe routing. Restart it, observe re-addition.

---

### 3.5 — Prefix-Aware Routing

**Reading list:**
- [Consistent Hashing and Random Trees](https://www.cs.princeton.edu/courses/archive/fall09/cos518/papers/chash.pdf) — the original consistent hashing paper (short, readable)
- [Prompt Cache: Modular Attention Reuse for Low-Latency Inference](https://arxiv.org/abs/2311.04934) — how prefix-aware routing improves cache hit rates in multi-server setups

Route requests with the same system prompt to the same backend to maximize KV cache hits.

- Consistent hash on system message content
- Fall back to least-connections if target is overloaded (>2x average in-flight)

**Test:** 30 requests with system prompt A, 30 with B. Compare latency vs round-robin — prefix-aware should win on same-prefix requests.

Questions:
1. What's the trade-off between cache hit rate and load balance?
2. When would pure least-connections beat prefix-aware?

---

### 3.6 — Multi-Model Routing

Pull a second model:

```bash
docker model pull ai/llama3.2:3B-Q4_K_M
```

Run one llama-server instance with the 1B model, another with the 3B model. Add routing logic:
- If the request specifies a model, route to the matching backend
- If the model isn't available, return 404 with available models
- Add a `GET /v1/models` endpoint that aggregates across all backends

**Acceptance criteria:**
- Different models are served by different backends
- Client gets a unified model list from the gateway
- You can answer: how does model-aware routing interact with autoscaling? (scaling each model pool independently)

---

## Part 4: Deep Dive — Batching and PagedAttention

This is the most important section for understanding modern inference systems.

### 4.1 — Static vs Continuous Batching

> **Format:** Paper study + local experiment

Read the [Orca paper](https://www.usenix.org/conference/osdi22/presentation/yu) and understand why iteration-level scheduling matters.

**Local experiment:** You can observe continuous batching in llama-server directly. Run it with `-np 4` (4 slots) and fire 4 requests simultaneously with very different `max_tokens` (10, 50, 100, 200). Log when each response completes. Then compare with `-np 1` — the 4 requests now serialize. The difference in total wall-clock time is the benefit of continuous batching.

**Acceptance criteria — you can explain:**
- In static batching, why does the shortest request in a batch waste compute while waiting for the longest?
- How does continuous batching (iteration-level scheduling) solve this?
- What is the relationship between batch size and throughput? Between batch size and per-request latency?
- Draw the timeline of 4 requests with different lengths under static batching vs continuous batching

---

### 4.2 — PagedAttention and vLLM

> **Format:** Paper study + local experiment

Read the [PagedAttention paper](https://arxiv.org/abs/2309.06180) and explore [vLLM](https://docs.vllm.ai/).

**Local experiment:** You can observe KV cache memory pressure in llama-server. Run with a small context (`-c 512 -np 4` = 128 tokens per slot). Send requests with increasing prompt lengths and watch what happens when a prompt exceeds the per-slot context. Then run with `-c 4096 -np 4` and observe memory usage (`docker stats` or process RSS). The difference illustrates why KV cache memory management matters — and why PagedAttention's approach of allocating on-demand instead of pre-allocating is so impactful.

**Acceptance criteria — you can explain:**
- What problem does PagedAttention solve? (hint: KV cache memory fragmentation)
- How does paging work for KV cache? How is it analogous to OS virtual memory?
- What is the memory waste from internal/external fragmentation in naive KV cache allocation?
- How does prefix caching work in vLLM? How is it different from llama.cpp's approach?

---

### 4.3 — Run vLLM (Optional — requires GPU)

> **Format:** Hands-on (if you have a CUDA GPU), otherwise skip

If you have a GPU, install and run vLLM:

```bash
pip install vllm
vllm serve meta-llama/Llama-3.2-1B --dtype auto
```

vLLM exposes the same OpenAI-compatible API. Point your benchmark script at it.

**Acceptance criteria:**
- Compare tokens/sec between vLLM, llama-server, and Docker Model Runner for the same model
- You can answer: why is vLLM typically faster than llama.cpp for batched workloads? (hint: PagedAttention, CUDA kernels)
- Observe vLLM's `--max-num-seqs` (max batch size) — how does it affect throughput vs latency?

---

### 4.4 — Speculative Decoding

> **Format:** Research + local experiment

Research speculative decoding — a technique for faster inference without quality loss.

**Reading list:**
- [Fast Inference from Transformers via Speculative Decoding](https://arxiv.org/abs/2211.17192) — the original speculative decoding paper
- [Speculative Decoding — Hugging Face](https://huggingface.co/blog/whisper-speculative-decoding) — accessible explainer with diagrams of the draft-verify loop

**Local experiment:** llama-server supports speculative decoding via the `--draft` flag. Download a smaller model (e.g., Llama 3.2 1B as draft for a 3B target). Run `llama-server -m <3B-model> --draft <1B-model> -nd <num-draft-tokens>` and benchmark against the 3B model alone. Measure tokens/sec and observe the acceptance rate in the logs.

If you only have the 1B model, you can still experiment: llama.cpp also supports self-speculative decoding (`--draft-self`) where the model drafts for itself using fewer layers. Try it and measure the impact.

**Acceptance criteria — you can explain:**
- How does speculative decoding use a smaller "draft" model to speed up a larger "target" model?
- Why does it produce identical output to running the target model alone?
- What is the acceptance rate and how does it affect speedup?
- When does speculative decoding help most? When does it hurt?
- How would you integrate this into your gateway? (hint: model pair routing)

---

## Part 5: Fleet Orchestration

### 5.1 — Containerize the Gateway

Multi-stage Dockerfile for your Rust gateway:
- Build stage: compile with `cargo build --release`
- Runtime stage: minimal image (`debian-slim` or `distroless`) with just the binary

**Acceptance criteria:**
- Image size under 20MB
- `docker compose up -d` starts gateway + llama-server backends
- Gateway connects to backends via Docker internal network (service names)

---

### 5.2 — Compose Profiles

Add profiles so you can choose your backend:

- `docker compose --profile model-runner up -d` — gateway only, pointing at `model-runner.docker.internal`
- `docker compose --profile self-hosted up -d` — gateway + 3 llama-server instances

Both serve the same API on the same port.

Questions: what are the operational trade-offs? (startup time, resource usage, scaling, debugging)

---

### 5.3 — Autoscaling Signals

**Reading list:**
- [Prometheus Exposition Format](https://prometheus.io/docs/instrumenting/exposition_formats/) — the format your `/metrics` endpoint should produce
- [Autopilot: Workload Autoscaling at Google Scale](https://research.google/pubs/autopilot-workload-autoscaling-at-google-scale/) — how Google autoscales ML workloads (the kind of system you'd build at Anthropic)

Expose a `/metrics` endpoint (Prometheus format) reporting:
- Request queue depth
- In-flight requests per backend
- p99 latency (rolling 60-second window)
- Cache hit rate
- Per-backend prompt processing time (KV cache effectiveness signal)

Implement a simple autoscaler controller:
- If queue depth > threshold for 30 seconds → log `SCALE_UP`
- If all backends idle for 60 seconds → log `SCALE_DOWN`

**Acceptance criteria:**
- Your metrics endpoint returns valid Prometheus exposition format
- You can answer:
  - What metrics would you use to decide when to scale an inference fleet?
  - Why is queue depth alone insufficient? (hint: it doesn't distinguish between a burst and sustained overload)
  - What's the cold start cost of adding a new inference backend? How does model loading time affect scaling decisions?

---

### 5.4 — Deployment Strategies

> **Format:** Research + local simulation

You're deploying a new model version to a fleet serving production traffic.

**Reading list:**
- [Kubernetes Deployment Strategies](https://kubernetes.io/docs/concepts/workloads/controllers/deployment/#strategy) — blue/green and rolling updates in the system you'd likely use in production
- [Canary Releases — Martin Fowler](https://martinfowler.com/bliki/CanaryRelease.html) — concise overview of the pattern
- [Safe model deployment at Anthropic](https://docs.anthropic.com/en/docs/about-claude/models) — observe how Anthropic versions models (claude-3-5-sonnet-20241022 etc.) and think about what that implies for deployment infrastructure

Research and design strategies for:

1. **Blue/green deployment** — how do you cut over from model v1 to v2?
2. **Canary deployment** — how do you gradually shift traffic to the new model?
3. **Shadow deployment** — how do you test a new model against production traffic without serving its output?

**Local simulation:** You already have multi-model routing from 3.6. Simulate a canary deployment locally:
- Run two llama-server instances: one with Q8_0 ("v1"), one with Q4_K_M ("v2" — pretend it's a new model version)
- Add weighted routing to your gateway: 90% to v1, 10% to v2 (configurable via env var or API)
- Run your load test. Gradually shift weight: 90/10 → 70/30 → 50/50 → 0/100
- Log per-backend latency and token counts at each stage
- Implement a rollback trigger: if v2's p99 latency exceeds v1's by more than 50%, automatically shift all traffic back to v1

**Acceptance criteria — you can explain:**
- How would you implement canary routing in your gateway? (you built it)
- What metrics would you monitor during a canary rollout to decide whether to proceed or rollback?
- How do you handle requests mid-stream during a rollover? (hint: connection draining)
- What's the cost of running two model versions simultaneously? How does this affect fleet capacity?

---

### 5.5 — Multi-Accelerator Concepts

> **Format:** Research + local simulation

Research how large models are served across multiple GPUs and accelerator types.

**Reading list:**
- [Megatron-LM: Training Multi-Billion Parameter Language Models Using Model Parallelism](https://arxiv.org/abs/1909.08053) — introduces tensor and pipeline parallelism (training paper, but the same sharding applies to inference)
- [Efficiently Scaling Transformer Inference](https://arxiv.org/abs/2211.05102) — Google's analysis of how to partition models across TPUs for inference, covers all parallelism strategies
- [How GPUs Work — Nvidia](https://developer.nvidia.com/blog/cuda-refresher-reviewing-the-origins-of-gpu-computing/) — foundational GPU architecture if you need it
- [AWS Trainium / Inferentia overview](https://aws.amazon.com/machine-learning/trainium/) — an example of a non-GPU accelerator to understand what "hardware-agnostic" means in practice

**Local simulation:** You can't run multi-GPU tensor parallelism locally (without multiple GPUs), but you can simulate **pipeline parallelism** at the gateway level. llama-server's `-ngl` flag controls how many layers are offloaded to GPU (or in CPU-only mode, you can think of it as a layer split). Run two llama-server instances and imagine they each hold half the model layers — your gateway would need to chain them (request → instance 1 → instance 2 → response). Build a simple pipeline proxy that forwards through two backends sequentially and measure the latency overhead vs a single backend. This is a toy version of pipeline parallelism's communication cost.

**Acceptance criteria — you can explain:**
- **Tensor parallelism:** splitting a single layer across GPUs. When is this used? What's the communication overhead?
- **Pipeline parallelism:** assigning different layers to different GPUs. How does micro-batching help with pipeline bubbles?
- **Expert parallelism:** for Mixture-of-Experts models, routing experts to different GPUs
- How would your gateway need to change if each "backend" is itself a multi-GPU deployment?
- What does "hardware-agnostic" mean in practice? What's the abstraction layer between your gateway and GPU/TPU/Trainium?

---

### 5.6 — Full Stack Integration

Wire everything into one system:

```
Client → Rate Limiter → Priority Queue → Load Balancer (prefix-aware) → Backend Pool
                                                                           ├── llama-1 (1B model)
                                                                           ├── llama-2 (1B model)
                                                                           └── llama-3 (3B model)
```

Write a load test:
- 100 requests over 60 seconds
- Mix of priorities, prompt lengths, max_tokens, and target models
- 30% share a common system prompt
- Report: throughput, TTFT/TPOT, p50/p99 latency, cache hit rate, queue depths, per-backend request count

**Acceptance criteria:**
- Full system starts with `docker compose up -d`
- Load test runs and produces a comprehensive report
- You can identify the bottleneck in your system and explain how you'd address it at 10x scale

---

## Exercise Format Legend

Exercises are labeled by format:

- **Hands-on** (no label) — write code, build something, run it locally
- **Paper study + local experiment** — read a paper, then run a local experiment that demonstrates the concept
- **Research + local experiment** — study a topic you can't fully replicate locally, then run a simplified local experiment to build intuition
- **Research + local simulation** — study a topic, then simulate it by repurposing your existing local infrastructure

Every exercise produces either working code or written answers. Nothing is "just reading."

## How to Work Through This

1. **Parts 0–1 first** — understand what you're optimizing before building the optimizer
2. **Measure everything** — every exercise should produce numbers. If you can't measure it, you don't understand it
3. **Write actual answers to the questions** — in a doc, not in your head. They're interview prep
4. **The papers matter** — Orca and PagedAttention are foundational. Read them, don't just skim
5. **Rust is the point** — the job lists Rust. Writing the gateway in Rust builds directly relevant experience
6. **Run the experiments** — the local experiments in Parts 4–5 exist because observing a concept beats reading about it

Good luck. Start with 0.1.
