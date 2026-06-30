# Discovery: Testing

## Purpose

Detect the project's test framework, test commands, build commands,
and lint configuration.

## Scan Targets

| Pattern | What It Reveals |
|---------|----------------|
| `test/` / `tests/` / `__tests__/` / `*_test.go` | Test directory structure |
| `jest.config.*` / `vitest.config.*` | JS test framework config |
| `pytest.ini` / `setup.cfg` / `pyproject.toml [tool.pytest]` | Python test config |
| `package.json scripts.test` | Test command |
| `.eslintrc*` / `ruff.toml` / `.golangci.yml` | Lint configuration |
| `.github/workflows/*.yml` | CI test/build/lint commands |
| `coverage/` / `.coveragerc` / `lcov.info` | Coverage configuration |

## Tools

Use Glob to find config files, Read to parse them. Use Bash to verify
commands work (e.g., `npm test --help` or `pytest --co -q | head`).

## Output

Structured report:
- Test framework(s) and version
- Test command (exact command to run tests)
- Build command (exact command to build)
- Lint command (exact command to lint)
- Syntax check command per language
- Coverage tooling (if any)
- Approximate test count
