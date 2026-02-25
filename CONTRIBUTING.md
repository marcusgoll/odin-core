# Contributing

## Prerequisites

- Rust stable toolchain
- cargo

## Local checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Compatibility policy

- Do not break plugin protocol in patch releases.
- Any behavior change in policy decisions requires regression tests.
- Keep compatibility runtime behavior stable unless explicitly version-gated.
