# Toolchain Contract

`tsnative doctor` verifies the minimum local environment before compilation:

- Rust compiler available on `PATH`.
- Clang available on `PATH`.
- The host target resolves to `x86_64-apple-darwin` in the current MVP workspace.
- The `tsnative-support/core/main.c` startup wrapper exists.

The command exits nonzero when a required tool or support file is missing. Version output is included in successful diagnostics so CI failures can be reproduced.
