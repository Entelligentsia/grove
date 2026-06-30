# Discovery: Routing

## Purpose

Detect the project's API surface, authentication strategy, and middleware.

## Scan Targets

| Pattern | What It Reveals |
|---------|----------------|
| Django: `urls.py`, `views.py` | URL patterns, view functions/classes |
| Express: `router.get/post/...` | Route definitions |
| Rails: `routes.rb` | Route definitions |
| FastAPI: `@app.get/post` | Endpoint decorators |
| Go: `http.HandleFunc` / mux | Route handlers |
| Auth decorators/middleware | `@login_required`, `authenticate`, `IsAuthenticated` |
| API docs: `openapi.yaml` / `swagger.json` | API specification |

## Tools

Use Grep to find route definitions and auth patterns, Read to parse them.

## Output

Structured report:
- Route count (approximate)
- Auth strategy (decorator-based, middleware, JWT, session, etc.)
- Public vs protected route ratio
- URL namespace structure
- API versioning (if any)
- Middleware chain
