# Target and ABI Policy

The compiler accepts canonical target triples from its first implementation. The initial supported targets are:

- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

The host target in this workspace is `x86_64-apple-darwin`. Code generation and support-library selection must consume the resolved target rather than parse CLI strings independently.
