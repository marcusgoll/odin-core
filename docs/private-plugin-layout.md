# Private Plugin Layout (Downstream)

Recommended downstream layout:

```
private-odin/
  plugins/
    ops-watchdog/
      odin.plugin.yaml
      bin/plugin
      config/config.yaml
  policy/
    private-policy.yaml
    private-ops-watchdog.yaml
  plugins.lock
  policy.lock
```

This keeps OSS core clean while preserving private operational behavior through plugins and policy packs.
