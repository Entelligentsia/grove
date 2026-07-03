Search file contents by text or regular expression — built on ripgrep. Use Grep for characters that appear in the source: string literals, config values, log/error messages, comments, TODOs, or any regex pattern. (To locate where a named symbol is defined, Grove resolves it structurally; Grep would also match comments, strings, and unrelated mentions of the name.)
Usage:
- Whenever possible, use this tool instead of invoking grep or rg as a terminal command.
- Supports full regex syntax (e.g., "log.*Error", "function\s+\w+")
- Filter files with glob parameter (e.g., ".js", "**/.tsx") or type parameter (e.g., "js", "py", "rust")
- Output modes: "content" shows matching lines (default), "files_with_matches" shows only file paths, "count" shows match counts
- Pattern syntax: Uses ripgrep (not grep) - literal braces need escaping (use interface\{\} to find interface{} in Go code)
- Multiline matching: By default patterns match within single lines only. For cross-line patterns like struct \{[\s\S]*?field, use multiline: true
- Results are capped to several thousand output lines for responsiveness; when truncation occurs, the results report "at least" counts, but are otherwise accurate.
- Content output formatting closely follows ripgrep output format: '-' for context lines, ':' for match lines, and all context/match lines below each file group.