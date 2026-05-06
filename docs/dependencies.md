# Mapa de Dependencias

Uma referencia chapada de tudo que o Historiador Doc precisa para
rodar em producao. Use junto com
[docs/installation.md](installation.md) (guia procedural) e
[docs/architecture.md](architecture.md) (diagrama de componentes).

## 1. Requisitos do host

| Recurso | Minimo (producao) | Recomendado | Notas |
|---|---|---|---|
| Sistema operacional | Linux x86_64 | Debian 12 ou Ubuntu 24.04 | macOS / Windows sao apenas para dev |
| CPU | 2 vCPU | 4 vCPU | Embeddings via Ollama se beneficiam de mais cores |
| RAM | 4 GB | 8 GB+ | Indice HNSW do Chronik + buffers do Postgres |
| Disco | 20 GB | 50 GB+ SSD | Postgres + dados vetoriais do Chronik + modelos Ollama |
| Docker Engine | 24+ | latest | Plugin Compose v2 obrigatorio |
| GPU | nenhuma | NVIDIA + Container Toolkit | So se rodar Ollama localmente; LLMs em nuvem nao precisam |
| Rede de saida | sim | — | Para puxar imagens e alcancar o provedor LLM |
| Rede de entrada | porta 443 | — | Para o seu proxy reverso |

Para deploys nao-Docker (bare-metal / Kubernetes), veja §6.

## 2. Toolchain de build

So necessario se voce construir as imagens voce mesmo em vez de
puxar imagens prontas.

| Ferramenta | Versao necessaria | Fixada em |
|---|---|---|
| Rust | stable, edition 2021, MSRV `1.82` | [`rust-toolchain.toml`](../rust-toolchain.toml), [`Cargo.toml`](../Cargo.toml) |
| Componentes Rust | `rustfmt`, `clippy` | `rust-toolchain.toml` |
| Node.js | 20+ | [`apps/web/Dockerfile`](../apps/web/Dockerfile) |
| pnpm | 10.30.1 (casa com o campo `packageManager`) | [`package.json`](../package.json) |
| Turbo | ^2.1.0 | `package.json` |
| openapi-typescript | ^7.4.0 | `package.json` |
| TypeScript | ^5.6.0 | `package.json` |
| Docker BuildKit | habilitado | `Dockerfile.rust` usa `# syntax=docker/dockerfile:1.7` |
| `cargo-chef` | embutido na imagem base `lukemathwalker/cargo-chef:latest-rust-1.93.1-bookworm` | `Dockerfile.rust` |

O workflow de CI em `.github/workflows/ci.yml` roda tres jobs em
paralelo: Rust (fmt → clippy → test → release build), Node (pnpm
install → lint), e Docker (smoke-build das imagens api/mcp/web).

## 3. Servicos em runtime

Estes rodam em containers sob `docker-compose.prod.yml`. Imagens
fixadas, portas e volumes estao listados abaixo.

### 3.1 PostgreSQL

| Aspecto | Valor |
|---|---|
| Imagem | `postgres:16.6-alpine` (prod), `postgres:16-alpine` (dev) |
| Porta do container | 5432 |
| Porta no host (prod) | nao bound — apenas rede interna |
| Porta no host (dev) | `127.0.0.1:5432:5432` (configuravel via `HOST_PORT_POSTGRES`) |
| Volume | `postgres_data:/var/lib/postgresql/data` |
| Init scripts | `docker/postgres/init/10-roles.sh` (cria roles app + mcp) |
| Healthcheck | `pg_isready -U historiador_admin -d historiador` |

**Roles** (pela [ADR-003](../artifacts/adr/ADR-003-mcp-server-architecture.md)):

| Role | Privilegios | Usado por | Variavel de connection-string |
|---|---|---|---|
| `historiador_admin` | superuser; roda DDL no boot inicial | apenas bootstrap | (nao usado em runtime pelo codigo da app) |
| `historiador_api` | CRUD completo nas tabelas da app | `apps/api` | `DATABASE_URL_READWRITE` |
| `historiador_mcp` | SELECT only em `workspaces, collections, pages, page_versions, chunks` | `apps/mcp` | `DATABASE_URL_READONLY` |

Migrations vivem em
[`crates/db/migrations/`](../crates/db/migrations/) e sao embutidas
no binario da api via `sqlx::migrate!`. Rodam em todo boot da api e
sao idempotentes.

### 3.2 Chronik-Stream

| Aspecto | Valor |
|---|---|
| Imagem | `ghcr.io/lspecian/chronik-stream:2.4.1` (configuravel via `CHRONIK_VERSION`) |
| Portas do container | 9092 (protocolo Kafka), 6092 (REST SQL / search) |
| Porta no host (prod) | nao bound — apenas rede interna |
| Porta no host (dev) | `127.0.0.1:9092` e `127.0.0.1:6092` |
| Volume | `chronik_data:/var/lib/chronik` |
| Healthcheck | `curl -fsS http://localhost:6092/health` |

**Topicos** (declarados via `CHRONIK_TOPICS`):

| Topico | Capabilities | Proposito |
|---|---|---|
| `published-pages` | vector, fulltext | Indice de chunks — a fonte da verdade do retrieval |
| `mcp-queries` | sql | Analytics em tool calls do MCP |
| `editor-conversations` | stream | Conversas do editor de IA v2 (Sprint 11) |
| `page-events` | stream, sql | Audit/eventing para CRUD de paginas |

Embeddings sao computados dentro do Chronik no momento de indexacao
usando `EMBEDDING_API_KEY` (encaminhado para o container como
`OPENAI_API_KEY` e `CHRONIK_EMBEDDING_API_KEY` — veja
[`docker-compose.yml`](../docker-compose.yml)). Sem essa chave, as
chamadas de produce funcionam mas o indice vetorial fica vazio e a
busca MCP nao retorna nada.

Veja [ADR-007](../artifacts/adr/ADR-007-chronik-stream.md).

### 3.3 Ollama (opcional)

So necessario se `LLM_PROVIDER=ollama`. Pule para LLMs em nuvem.

| Aspecto | Valor |
|---|---|
| Imagem | `ollama/ollama:0.5.7` (prod), `ollama/ollama:latest` (dev) |
| Porta do container | 11434 |
| Porta no host (prod) | nao bound — apenas interno |
| Porta no host (dev) | `127.0.0.1:11434` |
| Volume | `ollama_data:/root/.ollama` |
| Modelo default | `llama3.1:8b` (configuravel via `OLLAMA_MODEL`) |
| Modelo de embedding (dev) | `nomic-embed-text` |
| GPU | NVIDIA via `deploy.resources.reservations.devices` |

O container puxa o modelo configurado no primeiro boot — espere
downloads de varios GB.

### 3.4 historiador-api

| Aspecto | Valor |
|---|---|
| Imagem | `historiador/api:${IMAGE_TAG:-v1.0.0}` (override via `API_IMAGE`) |
| Construida a partir de | [`Dockerfile.rust`](../Dockerfile.rust) com `BIN_NAME=api` |
| Imagem base | `debian:bookworm-slim` (runtime), roda como uid 10001 |
| Porta do container | 3001 |
| Porta no host (prod) | nao bound — apenas interno |
| Depende de | `postgres` (healthy), `chronik` (healthy) |

### 3.5 historiador-mcp

| Aspecto | Valor |
|---|---|
| Imagem | `historiador/mcp:${IMAGE_TAG:-v1.0.0}` (override via `MCP_IMAGE`) |
| Construida a partir de | `Dockerfile.rust` com `BIN_NAME=mcp` |
| Imagem base | `debian:bookworm-slim`, roda como uid 10001 |
| Porta do container | 3002 |
| Porta no host (prod) | `${MCP_BIND_ADDR:-0.0.0.0}:${HOST_PORT_MCP:-3002}:3002` |
| Depende de | `postgres` (healthy), `chronik` (healthy), `api` (started) |

Este e o **unico** servico com bind de porta publica no host no
compose de producao — ver
[docs/architecture.md §fronteiras-de-seguranca](architecture.md#fronteiras-de-seguranca).

### 3.6 historiador-web

| Aspecto | Valor |
|---|---|
| Imagem | `historiador/web:${IMAGE_TAG:-v1.0.0}` (override via `WEB_IMAGE`) |
| Construida a partir de | [`apps/web/Dockerfile`](../apps/web/Dockerfile) (output standalone do Next.js) |
| Imagem base | `node:20-alpine`, roda como uid 1001 |
| Porta do container | 3000 |
| Porta no host (prod) | nao bound — alcancada via proxy reverso na rede interna |
| Depende de | `api` (started) |

## 4. Resumo de portas

| Porta | Bound onde (prod) | Servico | Proposito |
|---|---|---|---|
| 443 | host (proxy reverso) | nginx (sua responsabilidade) | TLS termination para `docs.example.com` e `mcp.example.com` |
| 3000 | apenas Docker interno | web | UI de autoria Next.js |
| 3001 | apenas Docker interno | api | REST API de autoria + Swagger UI em `/docs/` |
| 3002 | host (`0.0.0.0:3002`) | mcp | **A unica porta alcancavel externamente.** JSON-RPC do MCP + `/health` |
| 5432 | apenas Docker interno | postgres | SQL |
| 6092 | apenas Docker interno | chronik | REST DataFusion SQL + API de busca |
| 9092 | apenas Docker interno | chronik | Protocolo Kafka |
| 11434 | apenas Docker interno | ollama (se usado) | API HTTP do Ollama |

Sobrescreva binds no `.env` via `HOST_PORT_*` — ver
[.env.example](../.env.example).

## 5. Variaveis de ambiente

Lidas do `.env` pelo Docker Compose, pelos binarios Rust (via
`dotenvy`) e pelo Next.js (apenas `NEXT_PUBLIC_*`). Categorizadas
abaixo pelo servico que consome cada uma.

### 5.1 Obrigatorias no boot

O container da api se recusa a iniciar se qualquer uma destas
estiver ausente ou malformada.

| Variavel | Usada por | Formato / restricao |
|---|---|---|
| `JWT_SECRET` | api | ≥ 32 caracteres ASCII, usado como chave de assinatura HS256 |
| `APP_ENCRYPTION_KEY` | api | base64 de exatamente 32 bytes (`openssl rand -base64 32`) |
| `POSTGRES_ADMIN_PASSWORD` | postgres | senha forte |
| `POSTGRES_API_PASSWORD` | postgres + api | senha forte (deve diferir da admin) |
| `POSTGRES_MCP_PASSWORD` | postgres + mcp | senha forte (deve diferir da api) |
| `MCP_BEARER_TOKEN` | mcp | bearer token inicial (rotacionado depois pelo dashboard) |
| `PUBLIC_BASE_URL` | api | URL absoluta usada no `activation_url` de convites (ex: `https://docs.example.com`) |

### 5.2 Connection strings de banco

| Variavel | Usada por | Formato |
|---|---|---|
| `DATABASE_URL_READWRITE` | api | `postgres://historiador_api:${POSTGRES_API_PASSWORD}@postgres:5432/historiador` |
| `DATABASE_URL_READONLY` | mcp | `postgres://historiador_mcp:${POSTGRES_MCP_PASSWORD}@postgres:5432/historiador` |

Dois nomes distintos por design: reuso acidental se torna
sintaticamente impossivel.

### 5.3 LLM e embeddings

| Variavel | Usada por | Notas |
|---|---|---|
| `LLM_PROVIDER` | api | `"openai"`, `"anthropic"`, `"ollama"` ou `"test"` |
| `LLM_API_KEY` | api | Chave do provedor, ou URL base do Ollama quando o provedor e `ollama` |
| `EMBEDDING_API_KEY` | chronik | Chave de embedding compativel com OpenAI para o pipeline server-side do Chronik |
| `OLLAMA_MODEL` | ollama | Tag do modelo, default `llama3.1:8b` |
| `OLLAMA_GPU_COUNT` | ollama | `all` ou um inteiro; relevante apenas com NVIDIA |

Deploys com LLM apenas em nuvem nao precisam de `OLLAMA_*` e podem
parar o container ollama.

### 5.4 Chronik

| Variavel | Usada por | Default |
|---|---|---|
| `CHRONIK_VERSION` | docker-compose | `2.4.1` |
| `CHRONIK_KAFKA_BROKER` | api, mcp | `chronik:9092` (prod) / `localhost:9092` (dev) |
| `CHRONIK_SQL_URL` | api, mcp | `http://chronik:6092` (prod) / `http://localhost:6092` (dev) |
| `CHRONIK_SEARCH_URL` | api, mcp | igual a `CHRONIK_SQL_URL` |
| `ALLOW_IN_MEMORY_VECTOR_STORE` | api, mcp | **deve ser `false` em producao** |

### 5.5 Feature flags e knobs operacionais

| Variavel | Usada por | Default | Proposito |
|---|---|---|---|
| `BACKFILL_ON_BOOT` | api | `false` | Spawna task de startup que produz `page_versions` faltantes no Chronik |
| `EDITOR_V2_ENABLED` | api | `false` | Flag mestre do editor de IA v2 (Sprint 11) |
| `NEXT_PUBLIC_EDITOR_V2` | web | `false` | Espelho frontend da flag do editor v2 |
| `PROMPT_VERSION` | api | `v1` | Seleciona `prompts/agent/<version>.md` |
| `PROMPT_DIR` | api | `prompts/agent` | Sobrescreve o layout do diretorio de prompts |
| `RUST_LOG` | api, mcp | `info,sqlx=warn` | Filtro do `tracing` |
| `IMAGE_TAG` | docker-compose | `v1.0.0` | Versao da imagem para `historiador/{api,mcp,web}` |
| `MCP_BIND_ADDR` | docker-compose (mcp) | `0.0.0.0` | Endereco de bind para a porta MCP no host |
| `HOST_PORT_MCP` | docker-compose (mcp) | `3002` | Porta host-side do MCP |
| `HOST_PORT_POSTGRES` | docker-compose (postgres, so dev) | `5432` | Conveniencia de dev |
| `HOST_PORT_OLLAMA` | docker-compose (ollama, so dev) | `11434` | Conveniencia de dev |
| `HOST_PORT_CHRONIK_KAFKA` | docker-compose (chronik, so dev) | `9092` | Conveniencia de dev |
| `HOST_PORT_CHRONIK_SQL` | docker-compose (chronik, so dev) | `6092` | Conveniencia de dev |
| `API_INTERNAL_URL` | web | `http://api:3001` (prod) | Alvo dos rewrites `/api/*` |

### 5.6 Metadata de build

| Variavel | Usada por | Notas |
|---|---|---|
| `GIT_SHA` | api | Carimbada na resposta de `/health`; CI define, builds locais default para `unknown` |

## 6. Deploys nao-Docker

O compose de producao e o contrato canonico. Para deployar num
orquestrador nao-Docker (Kubernetes, Nomad, ECS, bare metal),
preserve os seguintes fatos load-bearing:

- Cada binario (api, mcp) e um unico ELF static-ish que escuta numa
  porta. Nao precisa de sidecar.
- Postgres e Chronik sao dependencias externas. Forneca as URLs deles
  via variaveis de ambiente (ver §5.2 e §5.4) e deixe os binarios
  conectarem.
- Configure o mesmo `JWT_SECRET` / `APP_ENCRYPTION_KEY` para toda
  replica da api ou as sessoes e as chaves LLM armazenadas quebram.
- Rode migrations uma vez por upgrade — todo boot da api faz isso de
  forma segura via `sqlx::migrate!`, mas voce pode pre-rodar com o
  CLI do `sqlx` para rollouts zero-downtime.
- Apenas o servico **mcp** deve estar voltado para a rede publica.
  Mantenha api e web em load balancers internos atras do seu auth.
- O servico web depende de `API_INTERNAL_URL` para os rewrites SSR —
  configure para a URL interna da sua api.

## 7. Servicos externos que voce pode precisar

| Servico | Quando obrigatorio | Proposito |
|---|---|---|
| OpenAI / Anthropic API | `LLM_PROVIDER=openai` ou `anthropic` | Geracao de texto para o editor de IA |
| API de embedding compativel com OpenAI | sempre (a menos que `LLM_PROVIDER=test`) | Usada pelo pipeline de embedding do Chronik (`EMBEDDING_API_KEY`) |
| ACME / certbot ou sua CA | sempre | Certs TLS para o proxy reverso |
| SMTP / SES / SendGrid | v1.1+ | Entrega nativa de email (hoje, convites retornam URL que o admin compartilha manualmente) |

## 8. Layout dos packages do workspace

Para contribuidores e autores de orquestrador que precisam saber o
que cada pacote constroi:

```
apps/
  api/          → bin: historiador_api → imagem historiador/api
  mcp/          → bin: historiador_mcp → imagem historiador/mcp
  web/          → app Next.js → imagem historiador/web
crates/
  blocks/       → lib: block tree do editor de IA (Sprint 11)
  chunker/      → lib: chunker de markdown structure-aware
  db/           → lib: clientes Postgres + Chronik, migrations
  llm/          → lib: abstracoes de provedor
  tools/        → lib: contrato de tool-calling do editor de IA (Sprint 11)
packages/
  types/        → tipos TypeScript gerados
```

Nomes de pacotes Rust usam underscores: `historiador_api`,
`historiador_mcp`, `historiador_db`, `historiador_chunker`,
`historiador_llm`. Binarios Cargo vivem em `apps/`; bibliotecas em
`crates/`. Veja [`Cargo.toml`](../Cargo.toml).

## 9. Referencias

- [docker-compose.prod.yml](../docker-compose.prod.yml) — imagens
  fixadas e variaveis de ambiente obrigatorias (a fonte da verdade).
- [docker-compose.yml](../docker-compose.yml) — apenas infra de dev.
- [.env.example](../.env.example) — template anotado de env.
- [Dockerfile.rust](../Dockerfile.rust) — multistage build para api/mcp.
- [apps/web/Dockerfile](../apps/web/Dockerfile) — build standalone do Next.js.
- [Cargo.toml](../Cargo.toml) — workspace, MSRV, deps compartilhadas.
- [package.json](../package.json) + [turbo.json](../turbo.json) —
  toolchain Node e graph de tasks.
