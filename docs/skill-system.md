# Skill System (SASS v0.1)

SASS v0.1 defines the canonical, state-aware skill contract used by Odin.

## Strict mode

SASS runs in strict mode by default:

- A skill must begin with `wake_up`.
- A skill must declare explicit end state(s).
- Every transition target must reference an existing state.
- Decision-state transitions must include guards.
- Non-end states must define `on_failure`.
- Skills must declare least-privilege permissions (`allowed_commands`, `allowed_plugins`, `network_policy`).

## Authoring format

- Schema: `schemas/skill-sass.v0.1.schema.json`
- Canonical examples: `examples/skills/sass/v0.1/*.skill.xml`

## Validation and diagram tooling

Validate a skill:

```bash
cargo run -p odin-cli -- skill validate examples/skills/sass/v0.1/run_tests.skill.xml
```

Compile XML to Mermaid:

```bash
cargo run -p odin-cli -- skill mermaid examples/skills/sass/v0.1/run_tests.skill.xml
```

## Breaking changes

- Legacy, non-state-aware multi-step skill definitions are not accepted by strict SASS validators.
- Skills missing `wake_up`, end states, guard coverage, or transition integrity fail validation.

## Migration path

1. Convert the skill to XML and add `wake_up` as the initial state.
2. Model state transitions explicitly, including `on_failure` for non-end states.
3. Add guards to decision branches.
4. Tighten permissions to least privilege.
5. Add a Mermaid diagram generated from the same XML.
6. Add a negative fixture to prove lint failure behavior.

## Verification matrix

- Schema contract:
  - `bash scripts/verify/sass-schema-contract.sh`
- Governance smoke:
  - `bash scripts/verify/sass-skill-governance-smoke.sh`
- CLI tests:
  - `cargo test -p odin-cli --test sass_skill_validate_cli -- --nocapture`
  - `cargo test -p odin-cli --test sass_skill_mermaid_cli -- --nocapture`
