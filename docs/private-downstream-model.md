# Private Downstream Model

Private Odin deployment composition:
- pinned `odin-core` version
- `plugins.lock`
- `policy.lock`
- private profile overrides

This model isolates private behavior outside OSS core while preserving deterministic upgrades and rollback.
