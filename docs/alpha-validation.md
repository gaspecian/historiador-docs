# Alpha Validation Test Script

End-to-end flow for validating the Historiador Doc Alpha release (`v0.1.0-alpha`).

## Prerequisites

- Docker and Docker Compose installed
- Ports 3000 (web), 3001 (API), 3002 (MCP), 5432 (Postgres) available
- An LLM API key (OpenAI or Anthropic), or use "Test" provider to skip

## Test Flow

### 1. Fresh Install

```bash
# Clone the repository
git clone <repo-url> historiador-doc
cd historiador-doc

# Start the stack
docker compose up
```

**Expected**: All services start without errors. Postgres initializes with roles and migrations.

### 2. Setup Wizard

1. Open `http://localhost:3000` in a browser
2. **Expected**: Auto-redirect to `/setup` (first-run detected via 423 response)
3. Complete the wizard:
   - Workspace name: `Test Workspace`
   - LLM Provider: select your provider (or "Test" for no LLM)
   - API Key: enter your key, click "Test Connection" (should show "Connection successful")
   - Languages: select primary language (e.g., `en`), optionally add others
   - Admin account: enter email and password (min 12 characters)
   - Review summary, click "Complete Setup"
4. **Expected**: Auto-login and redirect to `/dashboard/pages`

### 3. Create a Collection

1. In the left sidebar, click "+ New" next to "Collections"
2. Enter collection name: `Getting Started`
3. Click "Create"
4. **Expected**: Collection appears in the sidebar tree

### 4. Create a Nested Collection

1. Click "+ New" again
2. Enter name: `Tutorials` (this creates at root level)
3. **Expected**: Second collection appears in the sidebar

### 5. Create a Page via AI Editor

1. Click "New Page" button in the top-right
2. Enter title: `Welcome to Historiador`
3. In the AI editor, type a brief: `Write a welcome page for a documentation platform called Historiador Doc. Include sections about what it is, key features, and how to get started.`
4. Click "Generate Draft"
5. **Expected**: AI generates a markdown document (if using Test provider, a stub response)
6. Click "Save to page"
7. **Expected**: Redirect to the page detail view

### 6. Publish the Page

1. On the page detail view, click "Publish"
2. **Expected**: Status badge changes from "Draft" (yellow) to "Published" (green)
3. **Expected**: Chunking pipeline runs asynchronously (check API logs for chunk pipeline messages)

### 7. Verify Chunks

Check the API logs for messages like:
```
page_version_id = <uuid>, "async chunk pipeline completed"
```

Or query the database directly:
```sql
SELECT * FROM chunks WHERE page_version_id = '<version-id>';
```

**Expected**: One or more chunk rows with heading_path, section_index, and vexfs_ref populated.

### 8. Query via MCP

First, get the MCP bearer token from the Admin panel (`/dashboard/admin` > MCP Server > Regenerate Token).

```bash
curl -X POST http://localhost:3002/query \
  -H "Authorization: Bearer <your-mcp-token>" \
  -H "Content-Type: application/json" \
  -d '{"query": "What is Historiador Doc?"}'
```

**Expected**: JSON response with matching chunks from the published page:
```json
{
  "chunks": [
    {
      "content": "...",
      "heading_path": ["Welcome to Historiador", "What it is"],
      "page_title": "Welcome to Historiador",
      "score": 0.85,
      "language": "en"
    }
  ]
}
```

### 9. Admin Panel Validation

1. Navigate to `/dashboard/admin`
2. **Verify**: User list shows the admin account with "Active" status
3. **Verify**: MCP endpoint URL is displayed (e.g., `http://localhost:3002/query`)
4. **Verify**: Workspace config shows correct name, provider, and languages
5. Invite a user:
   - Enter email, select "Author" role, click "Invite"
   - **Expected**: Activation URL displayed with copy button
6. Regenerate token:
   - Click "Regenerate Token" and confirm
   - **Expected**: New token displayed with warning to save it

### 10. Search

1. Navigate to `/dashboard/pages`
2. Type the page title in the search bar
3. **Expected**: Page appears in filtered results

### 11. Language Completeness Badges

1. If workspace has multiple languages configured, check the page list
2. **Expected**: Green badge for languages with content, gray badge for missing ones

## Known Limitations (Alpha)

- **VexFS integration in progress**: Chunks persist only while the container is running. Container restart clears the in-memory vector store.
- **No email sending**: Invite activation links must be shared manually (copied from admin panel).
- **No page version history**: Edits overwrite the current version.
- **No Ollama embedding support**: If using Ollama as LLM provider, embeddings fall back to stub.

## Friction Points for Sprint 6

Document any issues encountered during validation here:

- [ ] _Issue description and steps to reproduce_
