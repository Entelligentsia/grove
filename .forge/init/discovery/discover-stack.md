# Discovery: Stack

## Purpose

Detect the project's programming languages, frameworks, versions, and runtime.

## Scan Targets

| File | What It Reveals |
|------|----------------|
| `package.json` | Node.js, npm dependencies, scripts |
| `requirements.txt` / `Pipfile` / `pyproject.toml` | Python, pip dependencies |
| `go.mod` | Go, module dependencies |
| `Cargo.toml` | Rust, crate dependencies |
| `Gemfile` | Ruby, gem dependencies |
| `Dockerfile` | Runtime, base image, build steps |
| `Makefile` | Build commands, task aliases |
| `*.sln` / `*.csproj` | .NET, C# |

## Tools

Use Glob to find these files, Read to parse them.

## Output

Structured report:
- Primary language(s)
- Framework(s) with versions
- Frontend framework (if any)
- Database (if detectable from dependencies)
- Task queue (if any)
- Containerisation (if any)
- Runtime version constraints
