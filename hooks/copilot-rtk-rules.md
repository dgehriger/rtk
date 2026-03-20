# RTK - Rust Token Killer (GitHub Copilot)

**Rule**: Always prefix shell commands with `rtk` to minimize token consumption.

```bash
rtk git status          rtk cargo test
rtk ls src/             rtk grep "pattern" src/
rtk gh pr list          rtk docker ps
```

Meta commands (no prefix needed):
```bash
rtk gain              # Token savings stats
rtk discover          # Find missed rtk usage
rtk proxy <cmd>       # Run raw, no filtering
```

The `.github/hooks/rtk-rewrite.json` hook intercepts raw commands automatically.
