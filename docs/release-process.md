# Release Process

1. Ensure CI is green on main.
2. Tag release: `vX.Y.Z`.
3. Release workflow produces artifact + checksum.
4. Publish changelog with compatibility notes:
   - plugin protocol changes
   - policy schema changes
   - migration and rollback notes
