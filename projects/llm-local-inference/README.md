# LLM Inference — Zero to Hero

A structured learning path for LLM inference systems engineering. Covers the full stack from transformer mechanics to fleet orchestration — the skills needed to build and operate inference infrastructure at scale.

**Language:** Rust (gateway)
**Build:** Buck2
**Runtime:** Docker Desktop with Model Runner, self-hosted llama.cpp

## Prerequisites

- Docker Desktop 4.40+ with Model Runner enabled
- Buck2 (available as `./buck2` at repo root)

### Enable Docker Model Runner

Open Docker Desktop > Settings > Features in Development > enable **Docker Model Runner**. Verify:

```bash
docker model --help
docker model list
```

### Reference Docs

Keep these open — they're used across multiple exercises:

- [Docker Model Runner docs](https://docs.docker.com/desktop/features/model-runner/)
- [OpenAI Chat Completions API reference](https://platform.openai.com/docs/api-reference/chat/completions)
- [llama.cpp documentation](https://github.com/ggerganov/llama.cpp)
- [llama-server documentation](https://github.com/ggerganov/llama.cpp/tree/master/examples/server)
- [Buck2 `genrule` docs](https://buck2.build/docs/prelude/globals/#genrule)

Papers and blog posts are linked inline in the exercises that use them.

---

## Part 0: How LLM Inference Works

Before writing any infrastructure code, understand what you're optimizing for. This part bridges "I've called the API" to "I know what happens between my request and the response."

### 0.1 — Pull a Model and Poke the API

Get a model running locally and explore what the API gives you.

```bash
docker model pull ai/llama3.2:1B-Q8_0
```

Send a non-streaming request and study the response:

```bash
curl http://localhost:12434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "ai/llama3.2:1B-Q8_0",
    "messages": [{"role": "user", "content": "What is 2+2?"}],
    "stream": false
  }'
```

Now send a streaming request (`"stream": true`, use `curl -N`) and watch the chunks arrive.

**Acceptance criteria:**
- `docker model list` shows the model
- You can explain what each field in the response means: `choices`, `usage.prompt_tokens`, `usage.completion_tokens`, `finish_reason`
- You've observed the difference between streaming and non-streaming — streaming returns chunks with `delta.content`, non-streaming returns the full `message.content`
- You can answer: why does the streaming response come in many small pieces instead of one big response? What determines the size of each piece?

---

### 0.2 — What Happens Inside: Tokens, Embeddings, Attention

You've used the API. Now understand what the model does with your request.

1. Read [The Illustrated Transformer](https://jalammar.github.io/illustrated-transformer/). Focus on: embedding, self-attention, the full encoder-decoder diagram. Then answer in your own words: what does self-attention compute and why is its cost quadratic in sequence length?
2. Read [What Is a Token?](https://platform.openai.com/tokenizer) — use OpenAI's tokenizer UI. Paste a few sentences and observe how text is split into tokens. Then answer: why is "tokenization" a separate step from the model? Why not feed raw characters?
3. Read [The Illustrated GPT-2](https://jalammar.github.io/illustrated-gpt2/). This is the decoder-only architecture that modern LLMs (including Llama) use. Focus on: how tokens are generated one at a time (autoregressive), and how the model attends to all previous tokens when producing the next one.

**Acceptance criteria — you can explain:**
- What is a token? Why do models work with tokens instead of words or characters?
- What is an embedding? What is positional encoding and why is it needed?
- What does self-attention do? Why is it the core operation?
- What does "autoregressive" mean? Why does the model produce one token at a time?
- What are "logits" and how does the model pick the next token? (hint: softmax → probability distribution → sampling)

---

### 0.3 — Prefill and Decode: The Two Phases

Now understand what makes inference expensive — and what you'll spend the rest of this project optimizing.

When you send "Explain Docker in one sentence" to the API, two very different things happen:

1. **Prefill** — the model processes your entire prompt at once (in parallel) and builds an internal data structure called the KV cache
2. **Decode** — the model generates tokens one at a time, reading from the KV cache on each step

These two phases have completely different performance characteristics.

1. Read [Transformer Inference Arithmetic](https://kipp.ly/transformer-inference-arithmetic/). This is the most important read in the entire project. Then calculate: for a 1B parameter model in FP16 (2GB), on your hardware's memory bandwidth, what's the theoretical max tokens/sec during decode? (formula: `memory_bandwidth_bytes_per_sec / model_size_bytes`)
2. Read the Databricks post [LLM Inference Performance Engineering](https://www.databricks.com/blog/llm-inference-performance-engineering-best-practices). Focus on the prefill vs decode analysis. Then answer: for your 1B model, is prefill or decode the bottleneck with a 500-token prompt and 100-token completion? What about 50-token prompt and 500-token completion?

**Acceptance criteria — you can answer:**
- Why is prefill compute-bound and decode memory-bandwidth-bound?
- What is the KV cache? Why is it needed? How does its memory grow with sequence length?
- What determines tokens/sec during decode? (hint: it's not FLOPs — it's how fast you can read model weights from memory)
- Why does a longer context window cost more memory but not proportionally more compute during decode?
- What's the difference between `prompt_tokens` and `completion_tokens` in cost? (hint: prompt tokens are processed in parallel during prefill; completion tokens are generated one by one)

---

### 0.4 — Quantization

You've been running `Q8_0`. Now pull a smaller quantization and compare.

```bash
docker model pull ai/llama3.2:1B-Q4_K_M
```

1. Read the [GGUF format spec](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md) — find the section on quantization types. What's the difference between Q4_0 and Q4_K_M? What does the "K" mean?
2. Read [Introduction to Quantization](https://mlabonne.github.io/blog/posts/Introduction_to_Weight_Quantization.html). Then calculate: a 1B model in FP16 is ~2GB. How large should Q8_0 and Q4_K_M be? Verify against the actual sizes Docker pulled.
3. Send the same 10 prompts to both Q4_K_M and Q8_0. Record tokens/sec and eyeball output quality differences.

**Acceptance criteria:**
- You can explain what quantization does (reduce weight precision from FP16 → INT8/INT4)
- You've measured tokens/sec and model size for Q4_K_M vs Q8_0 on the same hardware
- You can articulate the quality/speed/memory trade-off
- You can answer: when would you choose Q4 over Q8? When would neither be acceptable?
- Revisit your arithmetic from 0.3: does the theoretical decode speed change with quantization? (hint: smaller model = fewer bytes to read per token)

---

### 0.5 — Benchmarking Fundamentals

Before building anything, establish your measurement toolkit. Write a Rust CLI tool that:
- Sends requests to a configurable OpenAI-compatible endpoint
- Measures: time to first token (TTFT), time per output token (TPOT), end-to-end latency, tokens/sec
- Supports both streaming and non-streaming
- Runs N concurrent requests and reports p50, p95, p99

Create a Buck2 target for this tool. You'll reuse it throughout every exercise.

**Acceptance criteria:**
- `./buck2 run //projects/llm-local-inference:bench -- --url http://localhost:12434 --concurrency 10` works
- Outputs a clean table with TTFT, TPOT, throughput, and latency percentiles
- Run it against both Q4_K_M and Q8_0 — verify the numbers match your theoretical predictions from 0.3
- You can answer: why does TTFT increase with prompt length? Why does TPOT stay roughly constant regardless of prompt length? (hint: TTFT ≈ prefill time, TPOT ≈ single decode step)

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
- Run your bench tool from 0.5 against both — compare numbers

---

### 1.3 — Slots and Continuous Batching

`llama-server` uses **slots** — fixed context windows that requests are assigned to. This is its batching mechanism.

1. Read [How continuous batching enables 23x throughput](https://www.anyscale.com/blog/continuous-batching-llm-inference). Then draw the timeline of 4 requests (10, 50, 100, 200 tokens) under static batching vs continuous batching. Predict: how much faster should continuous batching be for this mix?
2. Read the [Orca paper](https://www.usenix.org/conference/osdi22/presentation/yu) (sections 1–4). Then verify your prediction by running the experiment below.

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

1. Read [How prompt caching works](https://sankalp.bearblog.dev/how-prompt-caching-works/). Then predict: for your 1B model, how much memory does the KV cache consume per token? Verify by running llama-server with `-c 512` vs `-c 4096` and comparing RSS.
2. Read [Dissecting Batching Effects in GPT Inference](https://le.qun.ch/en/blog/2023/05/13/transformer-batching/). Then answer: how does KV cache memory scale with batch size? With sequence length? Which dominates at high concurrency?
3. Read [Prompt Caching — Anthropic docs](https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching). This is the user-facing feature you'd be building. Note how it's exposed in the API. Then run the experiments below.

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

1. Read the [llama.cpp GBNF grammar guide](https://github.com/ggerganov/llama.cpp/blob/master/grammars/README.md). You'll need this syntax for the experiments below.
2. Read the [Outlines paper](https://arxiv.org/abs/2307.09702) (sections 1–3). Then answer: how does constrained decoding guarantee valid output without resampling or retrying?
3. Read [Structured Outputs — OpenAI docs](https://platform.openai.com/docs/guides/structured-outputs). This is the user-facing feature. Note the API design (how `response_format` works).

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

### 1.6 — Sampling Strategies

The `temperature` parameter is just one way to control generation. llama-server supports the full set of sampling parameters from the OpenAI API.

**Experiments:**
1. Send the same prompt 10 times with `temperature: 0`. Then 10 times with `temperature: 1.0`. Compare output variance.
2. Fix temperature at 1.0 and vary `top_p` (0.1, 0.5, 0.9, 1.0). How does it affect output diversity and quality?
3. Fix temperature at 1.0 and vary `top_k` (1, 10, 50). Same comparison.
4. Try `frequency_penalty: 1.0` and `presence_penalty: 1.0` — how do they change output for a prompt like "list 10 colors"?
5. Combine constrained decoding (from 1.5) with different sampling parameters. Does temperature affect JSON validity?

**Acceptance criteria:**
- You understand what each parameter does to the logit distribution before sampling
- You can explain: temperature scales logits, top-p truncates the distribution to a cumulative probability, top-k truncates to the K most likely tokens. What order are these applied in?
- You can answer: why does `temperature: 0` make output deterministic? What does `top_p: 0.1` mean in practice? Why is `top_k` rarely used in production APIs?
- You can answer: these are all features the inference team exposes through the API — where in the inference pipeline do they execute? (hint: after the model forward pass, before token selection)

---

### 1.7 — Model Loading and Cold Start

How long does it take for llama-server to start serving after launch? This is operationally critical — it affects autoscaling, deployment, and failover.

**Experiments:**
1. Time how long `llama-server` takes from process start to first successful response. Measure for both the 1B and 3B models.
2. Send a request immediately after starting llama-server. What happens? Does it queue, reject, or crash?
3. Kill a llama-server and restart it. How long before it can serve again?
4. Start llama-server with `--warmup` (if supported) or send a dummy request at startup. Does it change first-request latency?

**Acceptance criteria:**
- You've measured cold start time for each model size
- You can answer: why does loading a larger model take longer? (hint: reading weights from disk into memory)
- You can answer: how does cold start time affect autoscaling decisions? If scaling up takes 30 seconds, what's the minimum lead time your autoscaler needs?
- You can answer: how would you pre-warm a new backend before adding it to the load balancer pool?

---

### 1.8 — Containerize llama-server

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

Create a Buck2 target for the gateway. Look into `tokio`, `hyper` or `axum`, and `reqwest`.

**Acceptance criteria:**
- `./buck2 run //projects/llm-local-inference:gateway` starts the server
- Streaming and non-streaming requests both work through your proxy
- Your bench tool from 0.5 works against the proxy
- Proxy adds < 5ms overhead vs hitting Model Runner directly

---

### 2.2 — Request Queuing & Admission Control

1. Read [Little's Law](https://en.wikipedia.org/wiki/Little%27s_law). Then calculate: if your backend handles 5 req/sec with 2s average latency, what's the expected queue depth at 8 req/sec inbound? Verify with your bench tool.
2. Read [Performance Under Load — Netflix](https://netflixtechblog.medium.com/performance-under-load-3e6fa9a60581). You'll implement their adaptive concurrency limiter in exercise 2.7.

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

### 2.7 — Adaptive Concurrency Limiting

Your fixed concurrency limit from 2.2 is fragile — too low wastes capacity, too high causes latency spikes. Make it adaptive.

- Start with a concurrency limit of 5
- Track p99 latency over a rolling window (last 50 requests)
- If p99 rises above 2x your baseline → reduce limit by 1
- If p99 is healthy → increase by 1
- Bounds: 1–20

This is the Netflix pattern from the paper you read in 2.2.

**Test:** gradually ramp load from 1 to 30 req/sec. Log the concurrency limit over time — it should climb during healthy load and drop when latency spikes.

---

### 2.8 — Request Tracing

In production, you need to trace a request from client → gateway → backend → response. Add distributed tracing to your gateway.

- Generate a unique request ID for each incoming request (or use the client-provided `X-Request-ID` header)
- Propagate it to the backend via headers
- Include it in every log line (queue entry, backend selection, response start, response complete)
- Return it in the response headers

**Acceptance criteria:**
- You can grep logs for a single request ID and see its full lifecycle: arrival → queue wait → backend selection → first token → completion → total latency
- Under concurrent load, logs from different requests don't interleave ambiguously
- You can answer: why is request-level tracing essential for debugging tail latency issues in production?

---

### 2.9 — Error Handling and Timeouts

Backends fail. Requests hang. Handle it gracefully.

- Add a configurable timeout for backend requests (e.g., 30s). If the backend doesn't respond, return 504 to the client.
- If the backend returns a non-2xx status, forward the error to the client with your own error envelope (don't leak raw backend errors).
- If the backend disconnects mid-stream, close the client connection cleanly.
- Log all error events with the request ID from 2.8.

**Test:**
1. Start a request to llama-server, then kill llama-server mid-response. Does your gateway handle it cleanly?
2. Send a request with `max_tokens: 100000` (exceeds context). What does the backend return? What does your gateway return?
3. Set your timeout to 1 second and send a long prompt. Does the 504 fire correctly?

**Acceptance criteria:**
- No panics or connection leaks under any failure scenario
- Every error response includes the request ID and a useful error message
- You can answer: what's the right timeout for an inference request? (hint: it depends on max_tokens and decode speed — a 4000-token response at 30 tok/sec takes ~130s)

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

1. Read [Consistent Hashing and Random Trees](https://www.cs.princeton.edu/courses/archive/fall09/cos518/papers/chash.pdf) (short). Implement a consistent hash ring — you'll use it below.
2. Read [Prompt Cache: Modular Attention Reuse](https://arxiv.org/abs/2311.04934) (sections 1–3). Then predict: for 3 backends with 60 requests split 50/50 between two system prompts, what's the expected cache hit rate with consistent hashing vs round-robin?

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

## Part 4: Deep Dive — Inference Optimizations

This is the most important section for understanding modern inference systems.

### 4.1 — Static vs Continuous Batching

> **Format:** Paper study + local experiment

Read the [Orca paper](https://www.usenix.org/conference/osdi22/presentation/yu) — focus on sections 2–4. After reading, predict the throughput improvement you'd expect from iteration-level scheduling vs request-level scheduling for the workload below.

**Local experiment:** Verify your prediction using llama-server. Run it with `-np 4` (4 slots) and fire 4 requests simultaneously with very different `max_tokens` (10, 50, 100, 200). Log when each response completes. Then compare with `-np 1` — the 4 requests now serialize. The difference in total wall-clock time is the benefit of continuous batching.

**Acceptance criteria — you can explain:**
- In static batching, why does the shortest request in a batch waste compute while waiting for the longest?
- How does continuous batching (iteration-level scheduling) solve this?
- What is the relationship between batch size and throughput? Between batch size and per-request latency?
- Draw the timeline of 4 requests with different lengths under static batching vs continuous batching

---

### 4.2 — PagedAttention and vLLM

> **Format:** Paper study + local experiment

Read the [PagedAttention paper](https://arxiv.org/abs/2309.06180) — focus on sections 1–4. Then calculate: for your 1B model with 4 concurrent requests at 2048 context each, how much KV cache memory is wasted under pre-allocated (naive) allocation vs paged allocation?

**Local experiment:** Observe KV cache memory pressure in llama-server. Run with a small context (`-c 512 -np 4` = 128 tokens per slot). Send requests with increasing prompt lengths and watch what happens when a prompt exceeds the per-slot context. Then run with `-c 4096 -np 4` and observe memory usage (`docker stats` or process RSS). The difference illustrates why KV cache memory management matters — and why PagedAttention's approach of allocating on-demand instead of pre-allocating is so impactful.

**Acceptance criteria — you can explain:**
- What problem does PagedAttention solve? (hint: KV cache memory fragmentation)
- How does paging work for KV cache? How is it analogous to OS virtual memory?
- What is the memory waste from internal/external fragmentation in naive KV cache allocation?
- How does prefix caching work in vLLM? How is it different from llama.cpp's approach?

---

### 4.3 — FlashAttention

> **Format:** Paper study

Standard attention is quadratic in sequence length — O(n²) memory and compute. FlashAttention reformulates it to avoid materializing the full attention matrix, making it IO-aware.

1. Read [FlashAttention: Fast and Memory-Efficient Exact Attention](https://arxiv.org/abs/2205.14135) (sections 1–3). Focus on: why does standard attention waste GPU memory bandwidth? How does tiling fix this?
2. Read the follow-up [FlashAttention-2](https://arxiv.org/abs/2307.08691) (section 1–2). What changed?
3. Check whether your llama.cpp build uses FlashAttention: look for `--flash-attn` / `-fa` in `llama-server --help`. If available, benchmark with and without it.

**Acceptance criteria — you can explain:**
- Why is standard attention memory-inefficient? (hint: it materializes an N×N attention matrix)
- How does FlashAttention avoid this using tiling and recomputation?
- What is the difference between being "compute-bound" vs "IO-bound"? Which is standard attention during prefill?
- Why does FlashAttention help more with longer sequences?
- Every modern inference stack (vLLM, TensorRT-LLM, llama.cpp) uses FlashAttention or a variant. Why is it considered table-stakes?

---

### 4.4 — Run vLLM (Optional — requires GPU)

> **Format:** Hands-on (if you have a CUDA GPU), otherwise skip

If you have a GPU, run vLLM via Docker:

```bash
docker run --gpus all -p 8082:8000 vllm/vllm-openai --model meta-llama/Llama-3.2-1B --dtype auto
```

vLLM exposes the same OpenAI-compatible API. Point your bench tool at it.

**Acceptance criteria:**
- Compare tokens/sec between vLLM, llama-server, and Docker Model Runner for the same model
- You can answer: why is vLLM typically faster than llama.cpp for batched workloads? (hint: PagedAttention, CUDA kernels)
- Observe vLLM's `--max-num-seqs` (max batch size) — how does it affect throughput vs latency?

---

### 4.5 — Speculative Decoding

> **Format:** Research + local experiment

1. Read [Speculative Decoding — Hugging Face](https://huggingface.co/blog/whisper-speculative-decoding) for an accessible overview with diagrams.
2. Read the [original paper](https://arxiv.org/abs/2211.17192) (sections 1–3). Then calculate: if the draft model proposes 5 tokens and the acceptance rate is 70%, what's the expected speedup over standard decoding? Verify with the experiment below.

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
- Build stage: compile the release binary
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

1. Read the [Prometheus Exposition Format](https://prometheus.io/docs/instrumenting/exposition_formats/) spec. Your `/metrics` endpoint must produce this format.
2. Read [Autopilot: Workload Autoscaling at Google Scale](https://research.google/pubs/autopilot-workload-autoscaling-at-google-scale/). Then design your autoscaler signals based on what you learn about lag-based vs utilization-based scaling.

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

1. Read [Canary Releases — Martin Fowler](https://martinfowler.com/bliki/CanaryRelease.html). Then design: what metrics would you check before promoting a canary?
2. Read [Kubernetes Deployment Strategies](https://kubernetes.io/docs/concepts/workloads/controllers/deployment/#strategy). Then answer: why is a rolling update insufficient for model deployments? (hint: model loading time)
3. Look at [Anthropic's model versioning](https://docs.anthropic.com/en/docs/about-claude/models) (e.g. `claude-3-5-sonnet-20241022`). What does this naming imply about their deployment infrastructure?

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

1. Read [Megatron-LM](https://arxiv.org/abs/1909.08053) (sections 2–3) — it introduces tensor and pipeline parallelism. Then draw: how would you shard a 70B model across 8 GPUs using tensor parallelism? What about pipeline parallelism? What's the communication pattern for each?
2. Read [Efficiently Scaling Transformer Inference](https://arxiv.org/abs/2211.05102) — Google's analysis of partitioning for inference. Then answer: for a latency-sensitive chat API, would you prefer tensor or pipeline parallelism? Why?
3. Read [AWS Trainium / Inferentia overview](https://aws.amazon.com/machine-learning/trainium/). Then answer: what does "hardware-agnostic" mean in practice? What abstraction would your gateway need to treat GPU and Trainium backends interchangeably?

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

---

## Next Steps

After completing this project, here's where to go deeper.

### Read Source Code

The best way to understand production inference systems is to read them:

- **[vLLM](https://github.com/vllm-project/vllm)** — start with `vllm/engine/async_llm_engine.py` (request scheduling) and `vllm/core/scheduler.py` (the scheduler that implements continuous batching with PagedAttention). This is the most widely deployed open-source inference engine.
- **[llama.cpp server](https://github.com/ggerganov/llama.cpp/blob/master/examples/server/server.cpp)** — you built and used this. Now read how it handles slots, batching, and the request lifecycle.
- **[TensorRT-LLM](https://github.com/NVIDIA/TensorRT-LLM)** — Nvidia's inference stack. Heavier but shows how inference is optimized for specific hardware.
- **[SGLang](https://github.com/sgl-project/sglang)** — fast inference engine with RadixAttention (a tree-based prefix caching scheme). Interesting alternative to vLLM's approach.

### Papers

Foundational papers you should know for an inference role interview:

| Paper | Why it matters |
|-------|---------------|
| [Attention Is All You Need](https://arxiv.org/abs/1706.03762) | The transformer architecture — the thing you're serving |
| [FlashAttention](https://arxiv.org/abs/2205.14135) | IO-aware attention — you read this in 4.3 |
| [PagedAttention / vLLM](https://arxiv.org/abs/2309.06180) | KV cache memory management — you read this in 4.2 |
| [Orca](https://www.usenix.org/conference/osdi22/presentation/yu) | Continuous batching — you read this in 1.3 |
| [Speculative Decoding](https://arxiv.org/abs/2211.17192) | Draft-verify for faster decoding — you read this in 4.5 |
| [FlashDecoding](https://pytorch.org/blog/flash-decoding/) | Parallelizing attention across the KV cache during decode — complements FlashAttention |
| [Efficiently Scaling Transformer Inference](https://arxiv.org/abs/2211.05102) | Multi-device partitioning strategies — you studied this in 5.5 |
| [DistServe](https://arxiv.org/abs/2401.09670) | Disaggregating prefill and decode to different machines — a production technique at scale |
| [Sarathi-Serve](https://arxiv.org/abs/2403.02310) | Chunked prefills to prevent decode stalls — important for tail latency |

### Topics to Explore

- **Prefill-decode disaggregation** — DistServe and Splitwise show how to run prefill and decode on separate hardware. This is directly relevant to fleet orchestration at Anthropic's scale.
- **Mixture of Experts inference** — MoE models (like Mixtral) only activate a subset of parameters per token. This changes routing, memory, and parallelism strategies.
- **KV cache compression** — beyond quantization: techniques like [Scissorhands](https://arxiv.org/abs/2305.17118) and [H2O](https://arxiv.org/abs/2306.14048) that evict unimportant KV cache entries.
- **Kernel optimization** — writing CUDA kernels for custom attention, fused operations. Read [Triton](https://triton-lang.org/) tutorials to understand how FlashAttention-style kernels are written.
- **Model compilation** — `torch.compile`, XLA, and how they reduce inference latency by fusing operations.
- **Kubernetes operators for ML** — [KServe](https://kserve.github.io/website/), [Ray Serve](https://docs.ray.io/en/latest/serve/index.html) — how inference is orchestrated in production Kubernetes clusters.
