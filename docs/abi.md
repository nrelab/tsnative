# Target and ABI Policy

The compiler accepts canonical target triples from its first implementation. The initial supported targets are:

- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

The host target in this workspace is `x86_64-apple-darwin`. Code generation and support-library selection must consume the resolved target rather than parse CLI strings independently.

## Support ABI

The support ABI is versioned by `TSNATIVE_SUPPORT_ABI_VERSION` in `tsnative-support/core/runtime.h`. Version `1` exports:

- `double tsnative_main(void)` as the generated program entry point.
- `void tsnative_print_number(double)` for deterministic numeric output.

Generated code must use the host platform calling convention and must not assume that support-library internals are available beyond these declarations.
