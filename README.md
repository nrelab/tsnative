# TSNative

TSNative is an experimental ahead-of-time compiler for a deliberately small, statically typed TypeScript subset. It lowers supported source programs to typed HIR, checked CFG/SSA MIR, LLVM IR, and native executables without Node.js, V8, or a JavaScript interpreter.

The project currently targets the host macOS x86_64 environment and uses Clang as the native linker.

## Status

The end-to-end Fibonacci path is implemented:

```text
TypeScript -> syntax diagnostics -> typed HIR -> MIR verification -> LLVM IR -> native executable
```

The implementation is intentionally an MVP. The supported language is narrow, and several production phases are tracked as GitHub issues in `nrelab/tsnative`.

## Quick Start

Requirements:

- Rust 1.93.1, as pinned by `rust-toolchain.toml`
- Clang on `PATH`
- macOS x86_64 for the current native build path

Check the local toolchain:

```sh
cargo run -q -p tsnative-cli -- doctor
```

Check a source file:

```sh
cargo run -q -p tsnative-cli -- check tests/fixtures/fib.ts
```

Build and run the native fixture:

```sh
cargo run -q -p tsnative-cli -- build tests/fixtures/fib_main.ts
cargo run -q -p tsnative-cli -- run tests/fixtures/fib_main.ts
```

Expected output:

```text
55
```

Build to a custom path:

```sh
cargo run -q -p tsnative-cli -- build tests/fixtures/fib_main.ts --output target/fibonacci
```

Inspect generated LLVM IR:

```sh
cargo run -q -p tsnative-cli -- emit-ir tests/fixtures/fib_main.ts
```

Check deterministic output by building twice:

```sh
cargo run -q -p tsnative-cli -- repro-check tests/fixtures/fib_main.ts
```

## CLI

```text
tsnative <command> <file.ts> [--target <triple>] [-o <path>]
```

Available commands:

- `doctor`: validate Rust, Clang, target, and support-library prerequisites.
- `check`: parse, type-check, lower, and verify MIR.
- `emit-ir`: print target-aware LLVM IR.
- `build`: link a native executable through Clang.
- `run`: build and execute a native executable.
- `repro-check`: compare two isolated LLVM and executable builds byte-for-byte.

The executable name is `tsnative`; the Cargo package is named `tsnative-cli`.

## Supported Subset

The current subset supports:

- `number` with IEEE-754 `f64` semantics.
- `boolean`.
- Typed functions and recursion.
- Numeric and boolean literals.
- Arithmetic, comparisons, and strict equality.
- Function calls.
- `if` statements.
- Static target selection using canonical target triples.

Unsupported constructs are rejected with stable syntax or type diagnostics. Current rejected areas include `any`, dynamic imports, `eval`, `Proxy`, `Reflect`, exceptions, async/generators, loops, and dynamic object behavior.

See [docs/subset.md](docs/subset.md) for the semantic contract and [docs/abi.md](docs/abi.md) for the support ABI.
Background research is archived in [docs/research](docs/research/README.md).

## Development

Run formatting and all tests:

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo test --workspace
```

The workspace crates are organized as:

- `tsnative-syntax`: source parsing and syntax diagnostics.
- `tsnative-typecheck`: subset validation and typed HIR construction.
- `tsnative-hir`: typed high-level representation.
- `tsnative-mir`: CFG/SSA MIR, verifier, and reference evaluator.
- `tsnative-codegen`: LLVM IR emission.
- `tsnative-driver`: target triple handling.
- `tsnative-cli`: command-line orchestration.
- `tsnative-support`: minimal native startup and ABI support.

## Repository Plan

The implementation roadmap is recorded in [PLAN.md](PLAN.md). Production-grade phase work is tracked in GitHub issues, including parser recovery, module resolution, MIR verification, runtime ABI, target lowering, reproducibility, and release qualification.
