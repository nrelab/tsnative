# TSNative MVP Subset

TSNative starts with a deliberately small, statically typed TypeScript subset.

The initial semantic contract is:

- `number` uses IEEE-754 `f64` semantics.
- `boolean`, fixed-size tuples and fixed-shape records are supported.
- Functions, recursion, local variables, `if`, `switch`, and loops are supported.
- Imports are statically resolved.
- `console.log` and `print` are the only initial output intrinsics.
- `any`, dynamic imports, `eval`, `Proxy`, `Reflect`, exceptions, generators, promises, and threads are rejected.

Unsupported constructs must produce stable diagnostics with a source span. They are never silently lowered to a different meaning.
