# Historiador Doc

Plataforma de documentacao open-source e self-hosted onde cada base de conhecimento ja vem com um servidor [MCP](https://modelcontextprotocol.io/) integrado. Crie documentacao com um editor de IA, e qualquer ferramenta de IA (Claude, Cursor, ChatGPT, agentes customizados) pode consultar instantaneamente via o endpoint MCP — sem nenhum trabalho de integracao.

## O que torna diferente

- **Representacao dual** — as paginas sao escritas como markdown legivel por humanos *e* armazenadas como chunks estruturais em um vector store ([VexFS](https://github.com/lspecian/vexfs)). Autores nunca veem os chunks; ferramentas de IA nunca veem o markdown bruto.
- **MCP nativo desde o primeiro dia** — o endpoint MCP e um servico standalone e somente-leitura. Empresas expoem apenas a porta do MCP externamente, mantendo o app de autoria e a API internos.
- **Multilingual por padrao** — os idiomas obrigatorios sao configurados na instalacao e aplicados em toda a documentacao. O editor de IA solicita ao autor a criacao de conteudo em cada idioma configurado.
- **Self-hosted, dados ficam dentro da empresa** — roda via Docker Compose em um VPS Linux padrao (2 vCPU / 4 GB minimo). Sem dependencia de cloud.

## Arquitetura

Monorepo multi-linguagem: **backend Rust** (Axum) + **frontend Next.js**, orquestrados pelo Turborepo.

```
apps/
  api/          Axum REST API      (porta 3001, interna)
  mcp/          Axum MCP server    (porta 3002, exposta externamente)
  web/          Next.js dashboard  (porta 3000, interna)
crates/
  db/           Clientes compartilhados Postgres + VexFS
  chunker/      Chunker de markdown structure-aware (comrak AST)
  llm/          Abstracao de provedores LLM (OpenAI, Anthropic, Ollama)
packages/
  types/        Tipos TypeScript auto-gerados a partir do openapi.yaml
```

O **servidor MCP tem zero acesso de escrita** ao Postgres e ao VexFS — garantido tanto na camada de variaveis de ambiente quanto na camada de roles do banco. Veja [ADR-003](artifacts/adr/ADR-003-mcp-server-architecture.md).

## Inicio rapido

### Pre-requisitos

- [Docker Desktop](https://docs.docker.com/get-docker/) (ou Docker Engine + Compose v2)
- [Rust toolchain](https://rustup.rs/) (canal `stable`)
- [Node.js 20+](https://nodejs.org/) com [pnpm](https://pnpm.io/) (`corepack enable`)
- [sqlx-cli](https://crates.io/crates/sqlx-cli) (`cargo install sqlx-cli`)
- Git

### Configuracao

```bash
# Clonar o repositorio
git clone https://github.com/lspecian/historiador-doc.git
cd historiador-doc

# Instalar dependencias Node (turbo, openapi-typescript, etc.)
pnpm install

# Clonar e corrigir o VexFS (unica vez — nao existe imagem publicada upstream)
scripts/setup-vexfs.sh

# Criar o .env local a partir do exemplo
cp .env.example .env
# Edite o .env se as portas padrao (3000, 3001, 3002, 5432, 7680) colidirem
# com outros servicos na sua maquina — use as variaveis HOST_PORT_*.

# Compilar e iniciar toda a stack
docker compose up --build
```

Quando os cinco servicos estiverem saudaveis (`docker compose ps`):

| Servico  | URL                        | Finalidade                  |
|----------|----------------------------|-----------------------------|
| web      | http://localhost:3000      | Dashboard (Sprint 1: pagina de health check) |
| api      | http://localhost:3001      | REST API                    |
| mcp      | http://localhost:3002      | Endpoint MCP (exposto externamente) |
| postgres | localhost:5432             | Armazenamento relacional    |
| vexfs    | localhost:7680             | Vector store para embeddings de chunks |

Verificacao:
```bash
curl http://localhost:3001/health
# {"status":"ok","version":"0.1.0","git_sha":"unknown"}

curl http://localhost:3002/health
# {"status":"ok","version":"0.1.0","service":"mcp"}
```

> **Nota:** substitua os numeros de porta pelos valores `HOST_PORT_*` se voce os alterou no `.env`.

## Desenvolvimento

### Rust

```bash
cargo build --workspace           # compilar todos os crates
cargo test --workspace            # rodar testes
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check           # verificar formatacao (CI exige isso)
```

### Migrations do banco

As migrations ficam em `crates/db/migrations/` e sao embutidas no binario `api` via `sqlx::migrate!`. Elas rodam automaticamente na inicializacao do api. Para rodar manualmente:

```bash
sqlx migrate run --source crates/db/migrations \
  --database-url "postgres://historiador_admin:devpassword@localhost:5432/historiador"
```

### Codegen OpenAPI → TypeScript

O contrato da API flui das anotacoes Rust para tipos TypeScript:

```
anotacoes utoipa (Rust) → openapi.yaml → openapi-typescript → packages/types/generated/
```

Regenere apos alterar qualquer anotacao `#[utoipa::path]` ou `ToSchema`:

```bash
pnpm gen:types    # roda gen:openapi e depois build:types
```

Tanto o `openapi.yaml` quanto o `packages/types/generated/index.ts` sao commitados para que contribuidores possam ler o contrato da API sem rodar o build completo.

### Frontend

```bash
cd apps/web
pnpm dev          # servidor de desenvolvimento Next.js com hot reload
```

O servidor de desenvolvimento faz proxy das requisicoes `/api/*` para a API Axum via o rewrite configurado em `next.config.ts`.

## Contribuindo

### Pre-requisitos

Voce precisara do Rust toolchain e do Node.js/pnpm instalados localmente. O backend Rust usa Axum, sqlx e utoipa; o frontend usa Next.js com TypeScript e Tailwind.

### Fluxo de trabalho

1. Faca um fork do repositorio e crie uma branch a partir da `main`.
2. Faca suas alteracoes. Garanta que os seguintes comandos passem antes de abrir um PR:
   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace
   pnpm install && pnpm lint
   ```
3. Abra um PR contra `main`. O CI roda tres jobs em paralelo: lint/test/build do Rust, lint/build do Node, e smoke builds das imagens Docker.

### Decisoes de arquitetura

As decisoes estao documentadas como ADRs em [artifacts/adr/](artifacts/adr/). ADRs sao append-only — para mudar uma decisao, escreva uma nova ADR que substitui a anterior (como a [ADR-006](artifacts/adr/ADR-006-application-stack-rust.md) fez com a ADR-004). Nao edite ADRs aceitas retroativamente.

Invariantes-chave a respeitar:

- **MCP tem zero acesso de escrita.** A role `historiador_mcp` no Postgres tem grants de SELECT-only em um conjunto restrito de tabelas. Nunca conceda INSERT/UPDATE/DELETE.
- **Chunks sao structure-aware, nunca de tamanho fixo.** O chunker percorre a AST do markdown nos limites de headings (H1 → H2 → H3) e nunca divide no meio de uma secao. Veja [ADR-002](artifacts/adr/ADR-002-chunking-strategy.md).
- **OpenAPI e a unica fonte de verdade do contrato da API.** Nunca edite manualmente `openapi.yaml` ou `packages/types/generated/`. Sempre adicione anotacoes `#[utoipa::path]` e regenere.

### Convencoes de estrutura do projeto

- **Binarios** ficam em `apps/` (api, mcp, web).
- **Bibliotecas** ficam em `crates/` (db, chunker, llm).
- **Nomes de pacotes Rust** usam underscores (`historiador_api`, `historiador_db`).
- Planos de sprint ficam em `artifacts/sprints/`. Sao snapshots historicos — nao reescreva.

## Licenca

[AGPL-3.0-or-later](LICENSE)
