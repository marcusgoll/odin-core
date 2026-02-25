# Compat Regression Matrix (Pinned Baseline)

## Command

```bash
bash scripts/verify/compat-regression.sh --legacy-root /home/orchestrator/cfipros
```

## Checks Executed

1. `bash -n scripts/odin/keepalive.sh`
2. `scripts/odin/tests/backend-state-test.sh`
3. `scripts/odin/tests/backend-switch-events-test.sh`
4. `scripts/odin/tests/keepalive-failover-test.sh`
5. `scripts/odin/tests/keepalive-cooldown-test.sh`
6. `scripts/odin/tests/keepalive-antiflap-test.sh`
7. `scripts/odin/tests/odin-service-launcher-test.sh`
8. `scripts/odin/tests/spend-ledger-test.sh`

## Pass Criteria

- Every command exits 0.
- No compatibility adapter contract violations are introduced.
- Any failure blocks release/push until resolved.

## Notes

- This matrix intentionally focuses on stable, high-signal behavior contracts.
- Full test sweeps can run separately, but this matrix is the required release gate.
