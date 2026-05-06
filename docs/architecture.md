# Blueprint de Arquitetura

Esta e a visao operacional de como o Historiador Doc esta conectado:
quais servicos falam com quais, o que cada um possui, e quais
fronteiras de confianca o design protege. Para o *porque* de cada
decisao, siga os links de ADR — eles sao a fonte da verdade.

Documentos relacionados:
- [docs/installation.md](installation.md) — como instalar isso num host.
- [docs/dependencies.md](dependencies.md) — versoes, portas e variaveis de ambiente concretas.
- [docs/security.md](security.md) — postura de seguranca, separacao de roles, manuseio de segredos.
- [artifacts/adr/](../artifacts/adr/) — decisoes arquiteturais append-only.

## Forma geral

O Historiador Doc e uma plataforma de documentacao self-hosted com
uma propriedade incomum: toda base de conhecimento ja vem com um
servidor **Model Context Protocol** integrado que qualquer cliente
de IA pode consultar.

```
                    ┌──────────────────────────┐
                    │ Clientes de IA externos   │
                    │ (Claude Desktop, Cursor,  │
                    │  ChatGPT custom GPTs,     │
                    │  agentes internos…)       │
                    └────────────┬──────────────┘
                                 │  HTTPS + Bearer token
                                 │  JSON-RPC 2.0 (MCP 2025-03-26)
                          ─ ─ ─ ─┼─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
                     (unica superficie exposta externamente)
                                 │
                ┌────────────────▼──────────────────┐
                │      Proxy reverso (nginx)        │
                │      Termina TLS                  │
                │      docs.example.com → web       │
                │      mcp.example.com  → mcp       │
                └─────────────┬──────────┬──────────┘
                              │          │
                              │          │
       Rede interna do Docker │          │
       (sem binds de host)    │          │
                              │          │
              ┌───────────────▼─┐  ┌─────▼──────────┐
              │ web (Next.js)   │  │ mcp (Axum)     │
              │ apps/web        │  │ apps/mcp       │
              │ porta :3000     │  │ porta :3002    │
              │ UI de autoria   │  │ READ-ONLY      │
              └────────┬────────┘  └─────┬──────┬───┘
                       │ /api/* rewrite  │      │
                       ▼                 │      │
              ┌─────────────────┐        │      │
              │ api (Axum)      │        │      │
              │ apps/api        │        │      │
              │ porta :3001     │        │      │
              │ migrations,     │        │      │
              │ auth, CRUD,     │        │      │
              │ editor de IA,   │        │      │
              │ swagger /docs/  │        │      │
              └────┬───────┬────┘        │      │
                   │       │             │      │
                   │       └─────────────┼──┐   │
       (READWRITE) │   (READONLY)        │  │   │
                   ▼                     ▼  ▼   ▼
              ┌─────────────────┐  ┌────────────────────────┐
              │ Postgres 16     │  │ Chronik-Stream         │
              │ historiador     │  │ (vector + fulltext)    │
              │  - workspaces   │  │  - published-pages     │
              │  - users        │  │    (vector,fulltext)   │
              │  - collections  │  │  - mcp-queries (sql)   │
              │  - pages        │  │  - editor-conversations│
              │  - page_versions│  │  - page-events         │
              │  - chunks (meta)│  │ portas :9092 / :6092   │
              └─────────────────┘  └────────────────────────┘
                                        ▲
                                        │ embeddings via
                                        │ EMBEDDING_API_KEY
                                        │
                                   ┌────┴──────────────┐
                                   │ Provedor LLM      │
                                   │ (OpenAI /         │
                                   │  Anthropic /      │
                                   │  Ollama)          │
                                   └───────────────────┘
```

Algumas coisas para tirar deste diagrama:

- A **unica porta vinculada na interface publica do host e a 3002 (MCP)**.
  Todo o resto fica na rede interna do Docker — veja a divisao
  `expose:` (sem bind de host) vs `ports:` (bind de host) em
  [docker-compose.prod.yml](../docker-compose.prod.yml).
- O **servico mcp nao tem nenhuma aresta de escrita**: ele fala com
  Postgres na role SELECT-only e com endpoints somente-leitura do
  Chronik.
- **Embeddings sao computados dentro do Chronik**, server-side, usando
  `EMBEDDING_API_KEY`. O app nao carrega mais um cliente de embedding
  — veja a entrada do [CHANGELOG](../CHANGELOG.md) sobre a
  aposentadoria do EmbeddingClient.

## Responsabilidades dos servicos

### `apps/api` — backend de autoria (`historiador_api`)

Binario unico, ponto de entrada unico para tudo que envolve escrita:

- Roda migrations sqlx no boot ([crates/db/migrations/](../crates/db/migrations/)).
- Possui o fluxo de auth JWT: login, refresh, convite, troca de senha.
- Possui o CRUD para workspaces, collections, pages, page_versions.
- Dirige o editor de IA (Sprint 4 SSE em v1.0; WebSocket v2 atras de
  `EDITOR_V2_ENABLED` por
  [ADR-009](../artifacts/adr/ADR-009-websocket-transport-reaffirm.md)).
- Encripta a chave LLM armazenada do workspace com
  `APP_ENCRYPTION_KEY` antes de persistir.
- Emite OpenAPI via anotacoes `utoipa` — embutido no binario, servido
  em `/docs/`, e pre-cozido em [openapi.yaml](../openapi.yaml) para
  codegen.

Escuta na **:3001** internamente. Nao exposto a internet publica.

### `apps/mcp` — servidor MCP read-only (`historiador_mcp`)

Um binario Axum separado para que a fronteira de confianca seja
forcada em todas as camadas (processo, env, role do banco):

- Fala MCP JSON-RPC 2.0 em `POST /mcp`: `initialize`, `tools/list`,
  `tools/call`.
- Expoe uma unica ferramenta, `query`, com `query` (obrigatorio),
  `language` (BCP 47, opcional), `top_k` (1–20, default 5).
- Autentica com `Authorization: Bearer <token>` usando comparacao
  em tempo constante sobre o digest SHA-256 (ver
  [docs/security.md](security.md)).
- Le do Postgres pela role `historiador_mcp` (SELECT-only) e do
  endpoint de busca do Chronik.
- Tem zero capacidade de escrita — nao existe modulo que construa
  uma instrucao INSERT, UPDATE ou DELETE.

Escuta na **:3002**. Esta e a **unica** porta que o profile de
producao expoe no host. Veja
[ADR-003](../artifacts/adr/ADR-003-mcp-server-architecture.md).

### `apps/web` — UI de autoria (Next.js 16 + React 19)

- Renderiza o dashboard, o split-pane do editor, o setup wizard e as
  views de admin.
- Faz proxy de `/api/*` para `API_INTERNAL_URL` via um rewrite do
  Next.js (ver `apps/web/next.config.ts`) — o navegador nunca fala
  com a api diretamente.
- Todas as chamadas de API passam por `apps/web/lib/api.ts`, que
  cuida de injecao de JWT, refresh em 401, e o redirect do gate 423
  do setup.
- Construido com Tailwind 4. Modo de output standalone do Next.js
  significa que a imagem de producao e um unico processo
  `node server.js`.

Escuta na **:3000** internamente.

### `crates/db` — acesso a dados compartilhado

Possui o pool de conexoes Postgres, as migrations sqlx e o cliente
Chronik. Tanto `apps/api` quanto `apps/mcp` dependem dele mas
constroem seus proprios pools com suas proprias connection strings —
`DATABASE_URL_READWRITE` para a api, `DATABASE_URL_READONLY` para o
mcp. Os dois nomes sao deliberadamente distintos para que reuso
acidental seja sintaticamente impossivel.

### `crates/chunker` — chunker de markdown structure-aware

Percorre a AST do markdown (via `comrak`) nos limites de heading
(H1 → H2 → H3) e emite chunks que sao cientes de estrutura: blocos
de codigo, tabelas e listas sao atomicos e nunca divididos no meio
de uma secao. Cada chunk carrega o caminho de heading pai, a tag de
idioma e a versao da pagina de origem. Ver
[ADR-002](../artifacts/adr/ADR-002-chunking-strategy.md).

### `crates/llm` — abstracao do provedor LLM

Baseada em traits: `TextGenerationClient` e `ToolCallingClient`, com
implementacoes para OpenAI, Anthropic, Ollama, mais um stub para
testes. O binario da api escolhe a implementacao em runtime a partir
de `LLM_PROVIDER`. Embeddings nao ficam mais aqui — Chronik agora
possui esse pipeline.

### `crates/blocks`, `crates/tools` — building blocks do editor de IA v2

O modelo de block tree da
[ADR-010](../artifacts/adr/ADR-010-canvas-block-tree.md) e o contrato
de tool-calling da
[ADR-011](../artifacts/adr/ADR-011-llm-tool-calling.md). Estao
conectados na api via o websocket do editor; atras de
`EDITOR_V2_ENABLED` ate a Sprint 11 terminar.

### `packages/types` — tipos TypeScript gerados

Anotacoes `utoipa` no Rust → `openapi.yaml` → `openapi-typescript`
→ `packages/types/generated/index.ts`. Tanto o `openapi.yaml` quanto
o arquivo TS gerado sao commitados para que contribuidores possam
ler o contrato sem rodar o pipeline de codegen. **Nunca edite a mao
nenhum dos dois.**

## Posse dos dados: representacao dual

A arquitetura e construida em torno de uma separacao estrita:

| Dado | Fonte da verdade | Store derivado |
|---|---|---|
| Markdown da pagina, metadata, usuarios, workspaces, idiomas | **Postgres** | — |
| Texto dos chunks, embeddings vetoriais, indices full-text | (derivado) | **Chronik-Stream** |

- Autores so veem markdown servido pelo Postgres. A web UI nunca
  mostra chunks.
- Clientes de IA so veem chunks via MCP. Nunca veem o markdown bruto.
- O chunker e a ponte: consome uma linha de `page_version` e emite
  N chunks, que sao produzidos no topico Chronik `published-pages`.
  O pipeline server-side do Chronik passa pelo embedding API e
  indexa.

Perder o Chronik e recuperavel — re-indexa do Postgres. Perder o
Postgres nao e — faca backup. Veja
[docs/installation.md §8](installation.md#8-backups).

## Fronteiras de seguranca

Tres fronteiras concentricas protegem o sistema:

```
   ┌─────────────────────────────────────────────────────┐
   │ B1. Rede: apenas :3002 (MCP) bound na NIC publica   │
   └────────────┬────────────────────────────────────────┘
                │
                ▼
   ┌─────────────────────────────────────────────────────┐
   │ B2. Processo: binarios separados (api, mcp)         │
   │     mcp e um app Axum totalmente separado, sem      │
   │     rotas de escrita registradas                    │
   └────────────┬────────────────────────────────────────┘
                │
                ▼
   ┌─────────────────────────────────────────────────────┐
   │ B3. Banco: roles Postgres distintas                 │
   │     historiador_api  → CRUD completo                │
   │     historiador_mcp  → SELECT only em tabelas       │
   │                        whitelisted                  │
   └─────────────────────────────────────────────────────┘
```

Comprometer o token publico do MCP da acesso de leitura aos chunks e
nada mais: mesmo se o binario fosse explorado, a role do banco nao
consegue mutar estado. Veja [docs/security.md](security.md) para o
modelo de ameaca completo e a postura de auditoria de dependencias.

## Fluxos de requisicao

### Uma query de leitor (MCP)

1. Cliente de IA envia `POST /mcp` por TLS para o proxy reverso, com
   um Bearer token e um body JSON-RPC `tools/call` para a ferramenta
   `query`.
2. nginx encaminha para o container mcp em :3002, preservando
   `Authorization`.
3. mcp faz comparacao em tempo constante do digest SHA-256 do token
   contra o digest armazenado no Postgres (lido pela role
   SELECT-only).
4. mcp chama o endpoint de busca do Chronik com a string de query e
   o filtro de idioma; Chronik roda retrieval vetorial + full-text e
   retorna chunk IDs + scores.
5. mcp hidrata os chunks via Postgres SELECT, formata o resultado
   como resposta de tool-call MCP, e retorna ao cliente.

Alvo end-to-end: **p95 < 2 s para 1.000 queries sobre 10.000 chunks**
— ver [docs/performance.md](performance.md).

### Um autor publicando uma pagina

1. Autor edita markdown na web UI; `apps/web` faz PATCH na
   `page_version` via api (autenticada por JWT).
2. api persiste no Postgres, roda o chunker sobre o novo markdown e
   produz eventos de chunk no topico Chronik `published-pages`.
3. O pipeline de embedding embutido do Chronik pega os novos eventos,
   chama o embedding API com `EMBEDDING_API_KEY` e escreve vetores
   + entradas full-text.
4. O endpoint MCP pode retornar os novos chunks na proxima query —
   sem reindex manual.

Se `BACKFILL_ON_BOOT=true`, a api spawna uma task de background no
startup que produz no topico Chronik qualquer `page_versions` que
esteja faltando, tornando reparos no startup idempotentes.

### Uma instalacao de primeira vez

1. Operador sobe o `docker-compose.prod.yml`. A api boota, roda
   migrations, mas todos os endpoints exceto `/health`,
   `/setup/init`, `/setup/probe` e `/docs/` retornam **423 Locked**.
2. Operador abre `PUBLIC_BASE_URL`, web UI redireciona para `/setup`.
3. Setup wizard faz POST em `/setup/init` com nome do workspace,
   provedor LLM + chave, idiomas, credenciais de admin.
4. api valida a chave LLM (probe no endpoint `/models` do provedor),
   cria o workspace + usuario admin, e flipa a flag de setup.
5. Dali em diante o gate esta aberto e o dashboard renderiza.

## Invariantes criticas

Estas sao load-bearing — violar qualquer uma quebra a arquitetura.
Leia a ADR linkada antes de propor mudancas:

- **Servidor MCP tem zero acesso de escrita.** Processo, env e role
  do banco concordam.
  [ADR-003](../artifacts/adr/ADR-003-mcp-server-architecture.md)
- **Chronik = verdade de retrieval, Postgres = verdade de conteudo/metadata. Nao duplique.**
  [ADR-007](../artifacts/adr/ADR-007-chronik-stream.md)
- **Chunks sao structure-aware, nunca de tamanho fixo.**
  [ADR-002](../artifacts/adr/ADR-002-chunking-strategy.md)
- **Todo chunk carrega um campo `language` BCP-47.**
  [ADR-005](../artifacts/adr/ADR-005-multilingual-architecture.md)
- **OpenAPI e a unica fonte de verdade para o contrato da API.**
  Editar a mao `openapi.yaml` ou o TypeScript gerado e proibido.
- **`ALLOW_IN_MEMORY_VECTOR_STORE=true` e dev-only.** Producao deve
  fail-fast se Chronik for inalcancavel; o store em memoria perde
  silenciosamente os chunks a cada restart.

## Onde as decisoes vivem

ADRs sao append-only. Para mudar uma decisao, escreva uma nova ADR
que substitua a anterior — como a
[ADR-006](../artifacts/adr/ADR-006-application-stack-rust.md) fez com
a ADR-004 e como a
[ADR-007](../artifacts/adr/ADR-007-chronik-stream.md) fez com a
[ADR-001](../artifacts/adr/ADR-001-vector-database.md).

| Topico | Leia |
|---|---|
| Escolha do vector store | [ADR-007](../artifacts/adr/ADR-007-chronik-stream.md) (substitui [ADR-001](../artifacts/adr/ADR-001-vector-database.md)) |
| Estrategia de chunking | [ADR-002](../artifacts/adr/ADR-002-chunking-strategy.md) |
| Isolamento do servidor MCP | [ADR-003](../artifacts/adr/ADR-003-mcp-server-architecture.md) |
| Stack da aplicacao | [ADR-006](../artifacts/adr/ADR-006-application-stack-rust.md) (substitui [ADR-004](../artifacts/adr/ADR-004-application-stack.md)) |
| Arquitetura multilingual | [ADR-005](../artifacts/adr/ADR-005-multilingual-architecture.md) |
| Editor de IA (split pane, transporte, blocks, tools, propostas, modos de autonomia, outline, comments) | ADR-008 → ADR-016 |
