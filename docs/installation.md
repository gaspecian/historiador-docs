# Guia de Instalacao

Este documento descreve como instalar o Historiador Doc na sua propria
infraestrutura para uso em **producao**. Para desenvolvimento local,
veja o [quick-start no README](../README.md#inicio-rapido).

Documentos relacionados:
- [docs/architecture.md](architecture.md) — o que cada servico faz e como os dados fluem.
- [docs/dependencies.md](dependencies.md) — versoes, portas, variaveis de ambiente, requisitos de host.
- [docs/security.md](security.md) — postura de seguranca, separacao de roles, rotacao de segredos.
- [docs/deploy/nginx.conf](deploy/nginx.conf) — config de referencia do proxy reverso.
- [docs/deploy/release-checklist.md](deploy/release-checklist.md) — checklist de pre-flight para o corte v1.0.

## 1. Escolha o formato de deploy

O Historiador Doc suporta tres formas oficiais de execucao:

| Formato | Quando usar | Arquivo compose |
|---|---|---|
| **Producao (recomendado)** | Self-host single-host em VPS Linux | `docker-compose.prod.yml` |
| **Desenvolvimento** | Iteracao local — apenas infra em Docker, binarios no host | `docker-compose.yml` |
| **Orquestrador customizado** | Kubernetes, Nomad, ECS, etc. | Use as imagens publicadas diretamente |

O restante deste guia cobre o formato de producao. Para dev, siga o
README. Para Kubernetes, o compose de producao e a fonte da verdade
para variaveis de ambiente obrigatorias e contratos entre servicos.

## 2. Requisitos de sistema

Alvo minimo de producao: **2 vCPU, 4 GB RAM, 20 GB de disco** em
host Linux. Cargas maiores escalam pelo tamanho do indice vetorial
do Chronik e pelo numero de sessoes simultaneas no editor — ver
[docs/performance.md](performance.md).

| Componente | Necessario |
|---|---|
| Sistema operacional | Linux x86_64 (testado em Debian 12, Ubuntu 24.04) |
| Docker Engine | 24+ com plugin Compose v2 |
| Rede de saida | Para puxar imagens (`ghcr.io`, `docker.io`) e alcancar o provedor LLM |
| Rede de entrada | Porta 443 para o proxy reverso (TLS termination); ver §6 |
| GPU (opcional) | NVIDIA + Container Toolkit, se rodar Ollama localmente |

Se voce trouxer seu proprio Postgres ou cluster Chronik-Stream, os
requisitos do host caem para algo como 1 vCPU / 2 GB apenas para os
containers api/mcp/web.

## 3. Pre-flight: provisionar segredos

Estes sao os segredos que o compose de producao recusa-se a iniciar
sem. Gere uma vez e armazene no seu gerenciador de segredos.

```bash
# Segredo HMAC de 32 bytes para JWT (HS256)
openssl rand -base64 32        # → JWT_SECRET

# 32 bytes (codificados em base64) para AES-GCM da chave LLM armazenada
openssl rand -base64 32        # → APP_ENCRYPTION_KEY

# Senhas das roles Postgres (3 valores distintos)
openssl rand -base64 24        # → POSTGRES_ADMIN_PASSWORD
openssl rand -base64 24        # → POSTGRES_API_PASSWORD
openssl rand -base64 24        # → POSTGRES_MCP_PASSWORD

# Bearer token do MCP (rotacionado depois pelo dashboard, mas precisa de valor inicial)
openssl rand -base64 32        # → MCP_BEARER_TOKEN
```

**Por que tres senhas distintas para Postgres?** Pela
[ADR-003](../artifacts/adr/ADR-003-mcp-server-architecture.md), o
servidor MCP roda contra a role `historiador_mcp` (SELECT-only) num
subconjunto de tabelas, enquanto a API de autoria roda contra
`historiador_api` (CRUD completo). A role `historiador_admin` so roda
DDL no boot inicial. Reusar a mesma senha entre roles e uma
configuracao errada, nao uma conveniencia.

## 4. Instalar

### 4.1 Clonar o repositorio no host de deploy

```bash
git clone https://github.com/gaspecian/historiador-docs.git
cd historiador-docs
git checkout v1.0.0          # fixe em uma tag de release
```

### 4.2 Criar o `.env`

```bash
cp .env.example .env
```

Edite o `.env` e configure, no minimo:

```dotenv
# --- Roles Postgres (do §3) ---
POSTGRES_ADMIN_PASSWORD=<do-secret-manager>
POSTGRES_API_PASSWORD=<do-secret-manager>
POSTGRES_MCP_PASSWORD=<do-secret-manager>

# --- Segredos de auth (do §3) ---
JWT_SECRET=<minimo 32 caracteres>
APP_ENCRYPTION_KEY=<base64 de 32 bytes>

# --- MCP ---
MCP_BEARER_TOKEN=<do-secret-manager>

# --- URL publica que o dashboard envia em emails de convite ---
PUBLIC_BASE_URL=https://docs.example.com

# --- Provedor LLM (ver subsecao "Provedor LLM") ---
LLM_PROVIDER=openai
LLM_API_KEY=sk-...
EMBEDDING_API_KEY=sk-...      # exigido pelo pipeline de embeddings server-side do Chronik

# --- Rede de seguranca para producao ---
ALLOW_IN_MEMORY_VECTOR_STORE=false
```

A referencia completa de variaveis de ambiente, incluindo os knobs
opcionais, vive em
[docs/dependencies.md](dependencies.md#5-variaveis-de-ambiente).

> O binario da api se recusa a bootar se `JWT_SECRET` estiver ausente
> ou tiver menos que 32 caracteres, ou se `APP_ENCRYPTION_KEY` nao for
> um base64 valido de 32 bytes. Esse fail-fast e proposital — veja
> [apps/api/src/state.rs](../apps/api/src/state.rs).

### 4.3 Provedor LLM

O Historiador Doc suporta quatro provedores em `LLM_PROVIDER`:

| Valor | Com quem fala | Quando usar |
|---|---|---|
| `"openai"` | `https://api.openai.com/v1` | Default para a maioria dos deploys de prod |
| `"anthropic"` | `https://api.anthropic.com/v1` | Usuarios de Claude |
| `"ollama"` | Ollama local em `http://ollama:11434` | Deploys air-gapped ou BYO-GPU |
| `"test"` | Implementacao stub | Testes E2E, demos offline — nunca em prod |

Para self-hosted (Ollama), o compose de producao ja roda o servico
`ollama`. Para LLMs em nuvem, voce **nao** precisa do servico ollama
— pare com
`docker compose -f docker-compose.prod.yml stop ollama`, ou remova
o bloco do servico inteiramente.

O setup wizard valida o provedor antes de completar — ver §5.

### 4.4 Pull e start do stack

```bash
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d
```

No primeiro boot:

1. Postgres roda `docker/postgres/init/10-roles.sh` uma vez, criando
   as roles `historiador_api` e `historiador_mcp` e concedendo
   privilegios por tabela.
2. O container da api roda as migrations sqlx embutidas em
   `crates/db/migrations/`.
3. Chronik sobe saudavel com os topicos que a api espera
   (`published-pages`, `mcp-queries`, `editor-conversations`,
   `page-events`).
4. Os containers api, mcp e web sobem em ordem de dependencia.

Verifique se tudo esta saudavel:

```bash
docker compose -f docker-compose.prod.yml ps
docker compose -f docker-compose.prod.yml logs api | tail -50
```

Voce deve ver `Listening on 0.0.0.0:3001` da api e o log de migration
indicando que o schema esta atualizado.

## 5. Primeira execucao: completar o setup wizard

Ate o wizard rodar, todos os endpoints da API exceto `/health`,
`/setup/init`, `/setup/probe` e `/docs/` retornam **423 Locked**.

Abra `https://docs.example.com` (ou o que voce configurou em
`PUBLIC_BASE_URL`). O navegador e redirecionado para `/setup`. Siga:

1. **Workspace** — nome de exibicao para sua base de conhecimento.
2. **LLM** — selecione o provedor, cole a chave, clique em *Test
   Connection*. O probe atinge o endpoint `/models` (ou equivalente)
   do provedor com a chave.
3. **Idiomas** — escolha o idioma principal (BCP 47, ex: `pt-BR`) e
   quaisquer idiomas adicionais obrigatorios. Isso e permanente —
   adicionar idiomas depois exige migration manual. Ver
   [ADR-005](../artifacts/adr/ADR-005-multilingual-architecture.md).
4. **Admin** — email + senha (≥ 12 caracteres). Vira o primeiro
   usuario com role `admin`.
5. **Resumo** — revise e submeta.

Chamar `/setup/init` uma segunda vez retorna **409 Conflict**. Para
resetar num deploy fresco: `docker compose -f docker-compose.prod.yml down -v`
(isso destroi os volumes de Postgres e Chronik — irreversivel).

Para automacao (CI / IaC), o mesmo wizard pode ser dirigido via
`POST /setup/init` — ver o README para um exemplo com curl.

## 6. Proxy reverso + TLS

O Historiador Doc termina TLS num proxy reverso que voce traz. O
compose de producao expoe apenas a porta **3002** (MCP) no host —
todo o resto (api, web, postgres, ollama, chronik) fica na rede
interna do Docker. Isso casa com a fronteira de confianca da
[ADR-003](../artifacts/adr/ADR-003-mcp-server-architecture.md): o MCP
e a unica superficie alcancavel externamente.

Uma config nginx de referencia para dois virtual hosts (web UI + MCP)
fica em [docs/deploy/nginx.conf](deploy/nginx.conf). Ela:

- Termina TLS para `docs.example.com` (web) e `mcp.example.com` (MCP).
- Encaminha o header `Authorization` literalmente para que os bearer tokens do MCP sobrevivam.
- Desabilita request buffering em `/mcp` para o JSON-RPC permanecer rapido.
- Configura `Strict-Transport-Security`, `X-Content-Type-Options`, `X-Frame-Options`.

Voce provavelmente vai querer adicionar rate limiting e IP
allow-listing no virtual host do MCP. Ambos sao nginx de rotina; ver
os comentarios inline na config de referencia.

> A propria web UI e alcancada pelo proxy apenas na rede interna do
> Docker. Se voce precisar de uma topologia nao-padrao (ex:
> CloudFront na frente), troque o `expose: 3000` por
> `ports: 127.0.0.1:3000:3000` no servico `web` para que o proxy no
> host consiga alcanca-lo.

## 7. Conectar clientes MCP

Apos o setup, faca login no dashboard e va em **Admin → MCP Server**.
Clique em **Regenerate Token** para emitir um novo bearer token (isso
rotaciona o valor que a api armazena hashed; o token anterior e
invalidado).

Distribua o token + a URL publica do MCP (`https://mcp.example.com/mcp`)
para os seus clientes MCP. Para o Claude Desktop:

```json
{
  "mcpServers": {
    "historiador": {
      "url": "https://mcp.example.com/mcp",
      "token": "<bearer>"
    }
  }
}
```

Smoke-test do host de deploy:

```bash
curl -fsS https://mcp.example.com/health

curl -fsS -H "Authorization: Bearer $MCP_BEARER_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
     https://mcp.example.com/mcp | jq .
```

A versao de protocolo retornada deve ser `"2025-03-26"`.

## 8. Backups

Dois volumes stateful precisam de backup:

| Volume | Conteudo | Metodo de backup |
|---|---|---|
| `postgres_data` | Paginas (markdown), usuarios, workspaces, page versions, metadata | `pg_dump` contra a role `historiador_admin` |
| `chronik_data` | Vetores e indices full-text de chunks derivados do Postgres | Snapshot do volume; ou rebuild via `/admin/workspace/reindex` |

Postgres e a **fonte da verdade do conteudo** — perder o Chronik e
recuperavel (re-indexa do Postgres). Perder o Postgres nao e.

Cron recomendado:

```bash
docker compose -f docker-compose.prod.yml exec -T postgres \
  pg_dump -U historiador_admin historiador \
  | gzip > "/backups/historiador-$(date +%F).sql.gz"
```

Teste restores trimestralmente. Veja [docs/security.md](security.md)
para postura de divulgacao e resposta a incidentes.

## 9. Upgrades

Cada release fixa versoes em `docker-compose.prod.yml`. Para subir
de versao:

```bash
git fetch --tags
git checkout v1.1.0           # ou a release para a qual voce esta indo

# Re-le tags de imagem fixadas
docker compose -f docker-compose.prod.yml pull

# Restart rolling (containers voltam com a nova imagem)
docker compose -f docker-compose.prod.yml up -d
```

O container da api roda migrations sqlx em todo boot; sao
idempotentes. Verifique o [CHANGELOG](../CHANGELOG.md) por qualquer
release que exija acao manual do operador (ex: backfill de dados,
mudancas em variaveis de ambiente).

## 10. Health checks e endpoints operacionais

| Endpoint | Proposito | Auth |
|---|---|---|
| `GET /health` (api em :3001 interno) | Liveness | nenhuma |
| `GET /health/ready` (api) | Readiness — retorna 503 enquanto o backfill roda | nenhuma |
| `GET /health` (mcp em :3002 publico) | Liveness do MCP | nenhuma |
| `POST /mcp` (mcp) | O endpoint JSON-RPC do MCP | Bearer |
| `GET /docs/` (api em :3001 interno) | Swagger UI da API de autoria | nenhuma (mas inalcancavel externamente por padrao) |

Liveness probes para um orquestrador devem bater em `/health` em cada
container; readiness probes para a api devem usar `/health/ready`.

## 11. Problemas comuns na instalacao

| Sintoma | Causa | Correcao |
|---|---|---|
| `JWT_SECRET is required, min 32 chars` no boot | Segredo ausente ou curto | Configure no `.env`; regenere com `openssl rand -base64 32` |
| Erro de parse em `APP_ENCRYPTION_KEY` no boot | Nao e base64 de 32 bytes | Regenere com `openssl rand -base64 32` (a saida tem exatamente 32 bytes pre-encoding) |
| Todos os endpoints retornam 423 | Setup wizard nao rodou | Abra `PUBLIC_BASE_URL` no navegador; complete o wizard |
| MCP retorna 401 | Bearer token diverge | Confirme que `MCP_BEARER_TOKEN` casa com o valor usado pelo cliente; rotacione pelo dashboard se duvida |
| MCP retorna resultados vazios | `EMBEDDING_API_KEY` nao configurado, ou backfill nao rodou | Configure a chave, restart Chronik, depois chame `POST /admin/workspace/reindex` (admin) |
| Logs da api com `Chronik unreachable` | Servico Chronik nao saudavel | `docker compose ... ps chronik` — checar o container; **nunca** habilite `ALLOW_IN_MEMORY_VECTOR_STORE=true` em producao |
| `pg_isready` falha no primeiro boot | Script init ainda rodando | Espere — criacao inicial das roles pode levar 10–20 s |
