# Discovery: Processes

## Purpose

Detect the project's service topology, build tools, and deployment configuration.

## Scan Targets

| File | What It Reveals |
|------|----------------|
| `docker-compose.yml` | Service topology, dependencies, ports |
| `Procfile` | Process types (web, worker, etc.) |
| `ecosystem.config.js` | PM2 process management |
| `systemd/*.service` | System service definitions |
| `.github/workflows/*.yml` | CI/CD pipeline |
| `.gitlab-ci.yml` | CI/CD pipeline |
| `Jenkinsfile` | CI/CD pipeline |
| `vercel.json` / `netlify.toml` | Deployment platform |

## Tools

Use Glob to find these files, Read to parse them.

## Output

Structured report:
- Services and their roles (web, worker, database, cache, etc.)
- Build command(s)
- Deploy command(s) or platform
- CI/CD pipeline structure
- Environment variables referenced (names only, not values)
