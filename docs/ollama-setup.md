# Running Historiador Doc with Ollama (local LLM)

Use Ollama when you cannot send documentation content to a cloud LLM
API: air-gapped networks, regulated workloads, data-residency policies,
or simply avoiding API bills during internal trials.

Ollama is a standalone LLM runtime. Historiador Doc talks to it over
plain HTTP, so the install is a three-step job: run Ollama, pull
models, then point the setup wizard at the Ollama URL.

> Read [ADR-003](../artifacts/adr/ADR-003-mcp-server-architecture.md)
> for the MCP isolation model and
> [ADR-005](../artifacts/adr/ADR-005-multilingual-architecture.md) for
> the multilingual story. Neither changes under Ollama — only the LLM
> provider does.

---

## 1. Choose where Ollama runs

You have two placement options:

1. **Use the Ollama container in `docker-compose.yml` (easiest).** The
   provided compose file already ships an `ollama` service that binds
   to `127.0.0.1:11434` and pre-pulls `llama3.2:1b` on first boot. When
   the API container or `cargo run` picks `ollama` as the provider, it
   reaches it at `http://localhost:11434` (Linux/macOS) directly.
2. **Run Ollama on the host.** Preferred if you want GPU passthrough
   on Linux, or if you already have models cached locally. You start
   `ollama serve` on the host and leave the compose service out.

The rest of this guide covers both paths where they diverge.

---

## 2. Install Ollama (host-side option)

```bash
# Linux / WSL / macOS
curl -fsSL https://ollama.com/install.sh | sh

# Verify
ollama --version
ollama serve    # leave running in a terminal or as a systemd service
```

Windows: use the official installer from <https://ollama.com/download>.

---

## 3. Pull models

Historiador Doc needs two models:

- A **generation model** — answers the "Generate draft" / "Refine"
  prompts in the AI editor.
- An **embedding model** — produces the vectors that power MCP
  semantic search.

Sensible starter picks for a developer workstation:

```bash
# Generation. llama3.1:8b is a good balance of quality vs. RAM.
# For tiny machines fall back to llama3.2:1b.
ollama pull llama3.1:8b

# Embeddings. nomic-embed-text is 768-dimensional and widely supported.
# mxbai-embed-large (1024 dim) is higher-quality but larger.
ollama pull nomic-embed-text
```

Check what you have with `ollama list`. The setup wizard's **Test
connection** button calls `GET /api/tags` on your Ollama server and
populates two dropdowns from this list, so if a model is missing, pull
it and click the button again.

---

## 4. Docker networking (when the API runs in a container)

When the Rust API runs in a container and Ollama runs on the host, the
API cannot reach Ollama at `127.0.0.1` — that points at the container
itself.

- **macOS / Windows (Docker Desktop):** use `http://host.docker.internal:11434`.
  This hostname resolves automatically.
- **Linux:** `host.docker.internal` is not enabled by default. Add the
  following to the service that runs the API in
  `docker-compose.yml`:

    ```yaml
    extra_hosts:
      - "host.docker.internal:host-gateway"
    ```

  Then use `http://host.docker.internal:11434` as the base URL.

If you use the bundled `ollama` compose service, the API and Ollama
share the compose network and you can instead use
`http://ollama:11434` — no host-gateway trick needed.

---

## 5. Complete the setup wizard

1. Start the stack and open <http://localhost:3000>. The first-run
   wizard gates every API route with `423 Locked` until it runs.
2. On the **LLM Provider** step, pick **Ollama (local)** and fill in
   the base URL (typically `http://localhost:11434` for host-side, or
   `http://ollama:11434` if using the compose service from inside a
   container).
3. Click **Test Connection**. On success the wizard fetches
   `/api/tags` and shows the models you have pulled.
4. Select a **Generation model** (e.g. `llama3.1:8b`) and an
   **Embedding model** (e.g. `nomic-embed-text`). The two are
   independent — Ollama can serve both from the same server.
5. Complete the remaining steps (languages, admin account) and submit.

After setup, edit any of these values later in the admin panel under
**LLM Settings**.

---

## 6. Changing the embedding model later

Embedding dimensions are model-specific: `nomic-embed-text` is 768-d,
`mxbai-embed-large` is 1024-d, `text-embedding-3-small` is 1536-d. If
you ever switch the embedding model, existing stored vectors become
unreadable — the new query embeddings will not match them.

Historiador Doc warns you about this. When you save a new embedding
model in the admin panel, the response tells the UI how many published
page versions are affected and offers a **Re-index now** button. That
button spawns a background pass that walks every published page version
and re-embeds it with the new model.

Until the re-index finishes, MCP queries may return empty results. If
your workspace is large, plan a maintenance window.

Changing the **generation model** or the **provider** does not touch
embeddings and does not require re-indexing. It does require a restart
of the API process so the live `TextGenerationClient` picks up the new
configuration — the admin panel shows a banner when this is needed.

---

## 7. Known limitations vs. cloud LLMs

| Concern | Impact |
|---|---|
| No function calling | Ollama models do not expose tool/function-calling the way Anthropic and OpenAI do. Historiador's AI editor does not depend on this today, but future features that use tool-calling will fall back to a prompt-only flow when Ollama is active. |
| First-token latency | Larger Ollama models take several seconds to warm up, especially the first prompt after boot. `OLLAMA_KEEP_ALIVE=24h` (set in the compose file) keeps a pulled model hot. |
| Prompt sensitivity | Local models are more sensitive to prompt shape than frontier cloud models. If drafts look confused, try a larger model (`llama3.1:70b` if you have the RAM) or a more instruction-tuned variant. |
| Embedding quality | `nomic-embed-text` is good; it is not as strong as `text-embedding-3-small` on English technical content. If retrieval feels weak, switch to `mxbai-embed-large` and re-index. |
| No streaming tokens on the wire | The Ollama client implements NDJSON streaming internally and the API forwards it to the browser over SSE exactly like the cloud providers, so the editor UI renders tokens progressively. |

---

## 8. Troubleshooting

| Symptom | Probable cause |
|---|---|
| `ollama unreachable at <url>` during setup probe | Ollama is not listening, the URL is wrong, or Docker networking is missing `host.docker.internal`. Curl the URL from wherever the API runs: `curl -s <base>/api/tags` should return JSON. |
| Empty model dropdown after a successful probe | You have not pulled any models. Run `ollama pull <name>` and click **Test Connection** again. |
| MCP queries return nothing after changing the embedding model | The re-index has not been run or is still in progress. Check API logs for `workspace re-index complete`. |
| Drafts are very slow | Model is too large for the machine, or CPU-only inference on a big model. Swap to `llama3.2:1b` or `llama3.1:8b`. |
| `could not decrypt LLM key` in API logs for Ollama provider | Do not happen — Ollama deployments store the base URL in the clear. If you see it, the provider column disagrees with the stored secret. Reset via the admin panel. |
