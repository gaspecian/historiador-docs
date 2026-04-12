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
- Git

### 1. Infra (docker-compose)

Docker Compose roda apenas a infra (Postgres + Ollama). API, MCP e frontend rodam na maquina host para iteracao rapida.

```bash
# Clonar o repositorio
git clone https://github.com/lspecian/historiador-doc.git
cd historiador-doc

# Criar o .env local a partir do exemplo
cp .env.example .env
# Em producao, SEMPRE sobrescreva JWT_SECRET e APP_ENCRYPTION_KEY:
#   openssl rand -base64 32

# Subir Postgres + Ollama (modelo llama3.2:1b baixa automaticamente)
docker compose up -d
```

Na primeira execucao o Ollama baixa o modelo (~1.3 GB) em background. Acompanhe com `docker compose logs -f ollama`.

### 2. Backend (cargo run)

```bash
# Instalar dependencias Node (turbo, openapi-typescript, etc.)
pnpm install

# Rodar a API (le .env via dotenvy, aplica migrations no boot)
cargo run -p historiador_api

# Em outro terminal, se necessario:
cargo run -p historiador_mcp
```

| Servico  | URL                         | Finalidade                            |
|----------|-----------------------------|---------------------------------------|
| api      | http://localhost:3001        | REST API + Swagger UI em `/docs/`     |
| mcp      | http://localhost:3002        | Endpoint MCP (somente leitura)        |
| postgres | localhost:5432               | Armazenamento relacional              |
| ollama   | http://localhost:11434       | Inferencia local (Llama)              |

### 3. Frontend (pnpm dev)

```bash
cd apps/web
pnpm dev          # Next.js hot reload em http://localhost:3000
```

### 4. Primeiro uso: setup wizard

Ate o wizard rodar, todos os endpoints (exceto `/health`, `/setup/init` e `/docs/`) retornam `423 Locked`.

```bash
# Inicializar com Ollama local:
curl -X POST http://localhost:3001/setup/init \
  -H 'content-type: application/json' \
  -d '{
    "admin_email": "admin@example.com",
    "admin_password": "uma-senha-forte-aqui",
    "workspace_name": "Docs da Minha Empresa",
    "llm_provider": "ollama",
    "llm_api_key": "http://localhost:11434",
    "languages": ["pt-BR", "en-US"],
    "primary_language": "pt-BR"
  }'

# Login
ACCESS=$(curl -sX POST http://localhost:3001/auth/login \
  -H 'content-type: application/json' \
  -d '{"email":"admin@example.com","password":"uma-senha-forte-aqui"}' \
  | jq -r .access_token)

# Convidar outro usuario (v1 nao envia email — copie o activation_url)
curl -X POST http://localhost:3001/admin/users/invite \
  -H "authorization: Bearer $ACCESS" \
  -H 'content-type: application/json' \
  -d '{"email":"autor@example.com","role":"author"}'
```

> Para usar OpenAI ou Anthropic em vez de Ollama, passe `"llm_provider": "openai"` e `"llm_api_key": "sk-..."`.

Rodar o `/setup/init` duas vezes retorna `409 Conflict`. Para resetar (so em dev): `docker compose down -v`.

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
