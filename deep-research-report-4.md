# Executive Summary  
Recent advances show TypeScript (TS) can compile to lean static binaries without a JavaScript engine.  Vercel’s **scriptc** prototype compiles TS via the `tsc` type-checker into C and Clang, producing a simple program (e.g. a Fibonacci) in ~375 KB with ~12 ms startup. Similarly, **Perry** (a native TS compiler) uses SWC for parsing and LLVM for codegen to yield ~300 KB “Hello World” binaries with no Node/V8 and link only needed runtime code. These binaries start an order of magnitude faster than Node (e.g. 3–4 ms vs ~50 ms). Crucially, correctness is verified by diffing outputs: scriptc ensured 1,075 Node-based TS tests gave *identical* stdout/stderr and exit codes.  

Building a TS-native toolchain demands careful feature staging (see Roadmap below). We target **zero/minimal runtime** (static linking, no GC or optional RC, tight memory model) and **byte-identical deterministic outputs** across platforms.  Reproducible-build best practices (fixed timestamps, sorted sections, pinned tools) must be applied. Multi-OS/Arch support (x86_64, ARM64 on Linux/macOS/Windows) requires cross-compilation via LLVM/Clang or Rust/Musl toolchains. Advanced aspirations include GPU compute (via WebGPU/WGSL) and AI agent integration: e.g. TS libraries like TypeGPU show WebGPU shaders can be authored in TS and compiled to WGSL, and Google’s Agent DevKit illustrates multi-agent LLM APIs in TS.  

# Roadmap (Phases 1–3)  

1. **Phase 1 – Core Language:** *(Effort: Medium, Risk: Low)* Implement basics: TS parsing & type-check (via SWC or tsc), arithmetic, conditionals, loops, functions, recursion, basic types. Target static linking, no GC – use simple stack and heap, or reference counting for objects. Native integer/float replaces `number`, booleans to 1-bit. No runtime GC: memory for arrays/objects managed manually or via scoped arenas. Initial goal: “Hello, World” and numeric code (e.g. Fib) runs natively. Test against Node for bit-exact outputs.  

2. **Phase 2 – Extended Semantics:** *(Effort: High, Risk: Medium)* Add richer TS features: classes→structs, interfaces (compile-time only), arrays/tuples/vectors, maps/sets, string support, destructuring, spread, template literals, regex, enums, generics (erased), modules. Include minimal “stdlib” (e.g. basic I/O, JSON parsing). Implement closures (with closure-conversion) and simple async/Promise plumbing (or disallow heavy concurrency). Memory model: enable minimal runtime for heap-allocated objects (ref counting or a tiny GC), but strive for optional GC. Achieve small static binaries by linking only used parts (e.g. Perry: 300 KB hello by linking per-use). Cross-compile toolchains for Linux/Windows/macOS; use musl on Linux for portability.  

3. **Phase 3 – Full Ecosystem & Optimizations:** *(Effort: Very High, Risk: High)* Support complex features: exceptions, async/await fully, generators, big Node/POSIX APIs (`fs`, `net`, threads), optional JIT-like caching, optional dynamic fallback (like scriptc’s QuickJS tier). Address reproducibility: ensure deterministic builds (stable inputs, zero randomness). Deep optimization (inlining, tail-call elimination where possible), multithreading support with safety (Rust threads or OS threads; Perry provides `parallelMap` safely). Explore optional GC integration or memory arenas to support dynamic data. Integrate advanced “cutting-edge” features: fluid computing paradigms (dataflow/reactive async support), WebGPU binding libraries, and AI agents (e.g. embedding inference engines or calling LLM APIs). Throughout, mitigate risks: complex dynamic features may need either restriction or embedding a mini-runtime, and the project sustainability requires more than one developer (scriptc is one dev, past Vercel labs prototypes like *zerolang* stalled).

# TS Feature → Native Mapping  

| **TS Construct/Syntax**     | **Native Representation**                 | **Runtime Need**  | **Notes/Challenges**                                |
|-----------------------------|-------------------------------------------|-------------------|-----------------------------------------------------|
| **Primitives:** `number`    | 64-bit int/float (LLVM `i64`/`double`)    | None              | Map TS `number` to `i64` or `double`. Watch JS semantics (NaN, -0) if full JS compatibility.  |
| `boolean`                   | 1-bit or 8-bit (`i1`/`i8`)                | None              | Native boolean; operations lower to branches (no TS truthiness complexity). |
| `string`                    | Native UTF-8 sequence (pointer + length)  | Minimal           | Allocate storage; likely reference-count or arena-managed. Optimizable by immutability. |
| `undefined`/`null`         | Null pointer or optional flag              | Minimal           | Represent with sentinel (null pointer) or optional type; ensure checks. |
| **Variables/Bindings:**     | C variable/memory slot                    | None              | `const`→immutable, `let/var`→mutable memory. Block scope enforced by locals stack. |
| **Arithmetic & Logic:**     | Native ops (`add`, `sub`, `mul`, `and`, etc)| None           | Direct mapping; watch overflow/NaN rules. Use hardware FP for floats. |
| Comparisons (`<, >, ===`)*  | Native comparison (branch)                | None              | Implement strict `===` by comparing type tags and values. *Loose `==` normally disallowed (scriptc errors on `==`). |
| Control flow (`if/switch`)* | Native jumps                              | None              | Direct translation to branches.  |
| Loops (`for`, `while`, `for..of`)| Native loops (compile to branches)     | None             | `for..of` over arrays compiles to index loop. |
| **Functions/Calls:**        | Native functions (stack frame)            | None              | Support recursion naturally. Overhead minimal. Nested/arrow functions captured via closure structs (see below). |
| Arrow functions & closures  | Functions + context struct                | None              | Convert closures to structs with function pointer + env. Optimize by inlining small closures. |
| **Classes & Interfaces:**   | Structs with methods (vtable optional)    | Minimal           | Single-inheritance classes map to structs+methods; interfaces vanish at runtime. Virtual calls cost only if needed. |
| **Generics**                | Erased (monomorph or shared code)         | None              | Type parameters removed; specialize or use generic code without runtime impact. |
| **Enums**                   | Native enum/integer                       | None              | Map to integer constants.  |
| **Destructuring/Spread**    | Explode into native assignments/copies    | None              | Compile to temp variables/loops. |
| **Arrays/Tuples**           | Native array/struct types                 | Minimal           | Arrays=heap vector (pointer/len/capacity); tuples=structs. Manage memory. |
| **`Map`/`Set`**             | Native hash table (std::unordered_map)    | Minimal           | Include a minimal runtime lib for hash maps, or use C++ std containers (adds size). |
| **Typed Arrays**            | Flat C arrays or SIMD buffers             | None              | Map to fixed-size or pointer+length; no bounds checks unless added. |
| **`Promise`/`async`**       | State machine + threading or event-loop   | Full (or minimal) | Either compile `async` to state machines + spawn threads, or disallow/ require cooperative calls. Implement event loop? High effort. |
| **Generators**              | State machine + iterator protocol         | Full              | Complex; likely Phase 3 or disallowed. |
| **Exceptions (`throw`)**    | C++ exceptions or error codes             | Full              | Either use C++ exceptions (increases binary) or translate to error returns. |
| **`eval` / Dynamic code**   | *Not supported* or fallback to interpreter | N/A              | Scriptc disallows arbitrary `eval` in pure mode. Dynamic code would need embedded JS engine (scriptc tier-2). |
| **Type coercion (`as`,`<>`)**| Compile-time only (no-op)                 | None              | TS `as` cast is erased, but runtime checks could be added for safety (scriptc issues runtime cast error if mismatch). |
| **RegExp**                  | C regex (e.g. `std::regex`)               | Minimal           | Link in a regex library for `RegExp` support (scriptc Tier-1 includes `RegExp` handling). |
| **JSON**                    | Minimal parser (stdlib)                   | Minimal           | Could use a small JSON library for `JSON.parse/stringify`. |
| **StdLib/Host APIs**        | System calls/C stdlib or reimplement      | Depends           | Map Node APIs to OS: e.g. `fs`→posix calls, `net/http`→sockets. Perry implements ~95% of Node APIs natively.  

**Tier-1 vs Tier-2:** In practice, fully static compilation covers a *large, well-typed subset* (Tier-1). Scriptc reports it statically compiles classes, closures, generics, async/await, exceptions, regex and many Node APIs. Anything outside (e.g. untyped JS idioms) must use a dynamic fallback or be rejected. For example, scriptc purposely disallows loose `==` equality (error SC1040) and untyped loops, because they complicate zero-runtime guarantees.  

# Architecture & Build Pipeline  

 *Figure: Example Mermaid flowchart (code on left, rendered diagram on right). In a TS-native compiler we would design a multi-stage pipeline:* parse TS → type-check → lower to IR → optimize → generate machine code → link into native binary. For instance, a Solana blog illustrates a similar LLVM-based pipeline: frontends (Rust/Go/C) all emit LLVM IR, which is optimized and compiled to targets (x86, ARM, BPF, WASM, etc.). Likewise, our compiler’s IR/Optimizer/Codegen could be sketched in Mermaid (see figure).  

**Mermaid Pipelines:** We would graphically depict flows like a build pipeline or compiler passes.  For example, a flowchart might branch on static vs dynamic mode, include optimization passes, then target-specific codegen. In building/reproducibility workflows, we branch on OS/arch (Linux, macOS, Windows, x86_64/ARM64) and static linking flags.  A sample build flow might look like: (Start) → (check config) → [yes] static build → [no] dynamic build → End.  

 *Figure: Example flowchart (decision point with two branches). In our build pipeline we similarly branch on static vs dynamic linking, OS targets, etc.*  

# Toolchain & Implementation Choices  

- **Frontend & Parsing:** Use the TypeScript parser & type-checker (e.g. `tsc` or [TSGo](https://devblogs.microsoft.com/typescript/typescript-native-port/) rewrite), or a Rust TS parser like SWC. For strictness, leverage TS’s own type system.  
- **Intermediate Representation (IR):** Lower TS AST to a simple typed IR (High-level IR like Perry’s HIR), then to Static Single Assignment (SSA) IR. Optionally use **MLIR** to structure multiple lowering stages (unknown in TS context, but possible).  
- **Transforms:** Implement classic transforms: closure conversion, inline small functions, async-to-state-machine, tail-call optimization (where safe), and GC elision. Both Perry and scriptc perform inline/closure/async transforms before codegen.  
- **Backend:** LLVM is the natural choice for mature multi-arch codegen. It provides optimizations and supports x86/ARM static linking. Alternatively **Cranelift** (a Rust codegen) is faster to compile but has fewer optimizations; could use Cranelift for debug builds. We recommend LLVM (clang/LLD) for full performance and static linking. For quick prototypes, emitting C/C++ and invoking clang (as scriptc does) is viable but less flexible.  
- **Linker:** Use LLVM’s lld or system linker. For static binaries, pass `-static` or `-target` flags. On Linux use *musl* libc (via Alpine toolchain) for truly portable static linking; linking glibc statically is discouraged. On Windows use MSVC with `/MT` for static CRT or use MinGW (which links msvcrt by default). On macOS, true static linking of system libs isn’t possible (Apple provides no fully static libc), so link common libraries dynamically but bundle everything else.  
- **Language:** Implement the compiler in **Rust** or **Go**. Rust offers memory safety and mature LLVM/Cranelift integration; Go (like TS7) handles concurrency but lacks safe manual memory control. Given recent TS efforts (TS compiler rewrite in Go and SWC in Rust), both are options. A Rust-based tool can easily cross-compile and target multiple OSes. The resulting runtime library (if any) can also be in Rust/C for safety.  
- **Determinism:** Pin exact compiler/linker versions. Strip build IDs (`-Wl,--build-id=none`), use `-fno-ident` to remove timestamps. Use `SOURCE_DATE_EPOCH` for reproducible dates. Sort file lists in linking. Disable ASLR-dependent hashing. Use reproducible-builds guidelines: “Ensure stable inputs, stable outputs, capture minimal environment”. Store binaries in a canonical build directory to avoid path variability.  

# Reproducible Builds & Determinism  

To guarantee *byte-identical output* across builds and frontends, apply strict reproducibility practices. Lock tool versions (e.g. LLVM/Clang, libc). Use `-fno-ident` (no GCC/Clang version markers), `-Wl,--build-id=none`, and fix all timestamps (e.g. `SOURCE_DATE_EPOCH`). Ensure all archives (static libs) are built with deterministic options. Any RNG in the toolchain must be seeded. Avoid embedding filesystem paths. For cross-OS consistency (e.g. Linux vs Windows), verify outputs via hashing or diff. Scriptc explicitly tested byte-for-byte equivalence of compiled TS against Node/Go/Rust outputs (even using Zig for comparison).  

After compilation, embed version info or hashes as content if needed. To *verify* reproducibility, use diff tools on outputs from different platforms. Optionally, include a small metadata section in the binary (fixed by build-system) that can be checksummed. In distribution, users can run the TS-to-native compiler on the same sources and compare the binary hash with the published artifact to confirm fidelity.  

# Cross-Compilation & Static Linking  

| **OS/Arch**     | **Toolchain**               | **Static Strategy**                                     |
|-----------------|-----------------------------|---------------------------------------------------------|
| Linux x86_64    | LLVM with musl target       | Link against *musl libc* (fully static, portable on glibc systems). Alternatively, musl-gcc or `clang --target=x86_64-unknown-linux-musl -static`. Avoid glibc if static. |
| Linux ARM64     | LLVM (aarch64-*-musl)       | Same as x86_64: musl static (`aarch64-linux-musl-gcc`). Use CI with Alpine or cross-tools. |
| macOS x86_64/ARM64 | Apple Clang targeting macOS  | No fully static option; use `-mmacosx-version-min` to embed fallback. Link system libraries dynamically (libSystem). Only compile with `-static` if linking third-party libs you supply statically. |
| Windows x86_64  | MSVC (`cl.exe`) or clang-cl | Use `/MT` option for static CRT linking. All third-party libs must be in `.lib` form. MinGW-w64 can create 32-bit static EXEs (though it still links CRT dynamically). |
| Windows ARM64   | MSVC ARM64 toolchain       | Similar to x86_64 with `/MT`. |
  
Cross-building can be orchestrated via Docker (Alpine for Linux, Apple SDK inside macOS VM, mingw for Windows) or LLVM’s `--target` flags. Ensure building on a host matching musl/glibc. Windows static builds may require building on Windows host or using mingw/cross-tools for MSVC targets.  

**Static-Only Binary:** The goal is *one binary per platform*. This means bundling everything into the exe (no dynamic `.so`/`.dylib`). On Linux, musl makes this trivial. On Windows, using `/MT` bundles the C runtime. On macOS, true static is impossible for system libs, but you can ship a single `.app` bundle or static-link any non-Apple libs and rely on the system’s libSystem.  

# Security, Sandboxing & FFI/ABI  

- **Memory Safety:** By using Rust or careful C, eliminate unsafe patterns. If embedding a GC’d language (for dynamic fallback), isolate it in its own memory region or use a sandbox. Avoid JS’s `eval`; only allow FFI to vetted functions.  
- **Sandboxing:** For untrusted TS code (e.g. downloaded scripts or user plugins), use process isolation or employ WebAssembly runtime for sandbox. Otherwise, mark clearly which TS APIs are unsafe. Provide a limited “safe” global environment by default. Leverage OS features (seccomp on Linux) to restrict syscalls.  
- **FFI/ABI:** TS-native code uses the C ABI for interfacing. Expose TS functions as `extern "C"` if needed. For FFI to libraries (e.g. for system calls, GPU drivers), provide bindings (Rust crates or C headers). For Node compatibility, we must implement the Node API in our runtime (as Perry did) or provide a compatibility shim layer.  
- **Stack/Heap Bounds:** Ensure stack overflow/heap overflow are trapped. Use guard pages for stack. For heap, prefer safe allocators or reference-counting to avoid leaks.  
- **Concurrency:** If enabling threads, use native threads (POSIX or Windows threads). Provide TS primitives (`spawn`, `parallelMap`) that leverage threads with compile-time checks (as Perry promises). Avoid data races by default (use Rust’s ownership in APIs).  
- **Native ABI:** For each OS, follow system ABI (SysV on Linux, Win64 on Windows, Mach-O on macOS). Extern C calls must obey calling conventions.  

# WebGPU and Agent Integration  

To harness GPU acceleration, expose WebGPU/WGSL from TS. The [WebGPU API](https://www.w3.org/TR/webgpu/) (W3C standard) provides low-level GPU compute/graphics access. In practice, use a library like **wgpu** (Rust) or **Vulkan/Metal** under the hood. For example, the **TypeGPU** toolkit lets one write TS functions (tagged with `'use gpu'`) that are JIT-compiled to WGSL shaders. We can similarly allow TS developers to write compute kernels in TS that our compiler turns into GPU code. Provide TS primitives like `createComputePipeline` or WebGPU bindings as part of the TS standard library. Data (buffers/textures) would reside in GPU memory, with safe views in TS.  

For **agent frameworks**, integrate an AI/agent SDK. Google’s Agent DevKit (ADK) for TypeScript shows how to build multi-agent systems with LLMs like Google’s Gemma models. We could embed an LLM runtime or call a local server, exposing TS APIs for tool-calling and memory. Ensure safety by restricting LLM actions (structured APIs). Support scheduling/memory sharing via message passing or shared memory: e.g., TS tasks spawn lightweight agents (threads or async tasks) communicating through typed channels. Use the GPU for model inference if needed (via WebGPU compute). Optionally, allow WebAssembly modules as sandboxed “skills” called by TS agents. The TS compiler could emit code that ties into an agent scheduler, enabling asynchronous agent loops within the static binary.  

# Example: Compiling Fib(n)  

Consider `function fib(n: number): number { return n<2 ? n : fib(n-1) + fib(n-2); }`. A TS-native compiler lowers this to typed IR and then to machine code. For instance, LLVM IR might look like:  
```llvm  
define i64 @fib(i64 %n) {  
entry:  
  %cond = icmp slt i64 %n, 2         ; compare n<2  
  br i1 %cond, label %ret_base, label %recurse  

ret_base:  
  ret i64 %n                          ; return n for base case  

recurse:  
  %n1 = sub i64 %n, 1                 
  %call1 = call i64 @fib(i64 %n1)     ; fib(n-1)  
  %n2 = sub i64 %n, 2  
  %call2 = call i64 @fib(i64 %n2)     ; fib(n-2)  
  %sum = add i64 %call1, %call2  
  ret i64 %sum  
}  
```  
Optimizations: neither call is in tail position (since the add happens after the first call), so tail-call elimination isn’t applied here. A C/C++ backend would generate roughly:  
```c
int64_t fib(int64_t n) {
    if (n < 2) return n;
    int64_t a = fib(n-1);
    int64_t b = fib(n-2);
    return a + b;
}
```  
And on x86_64, GCC/Clang might compile this to:  
```
fib:  
    cmp    rdi, 1  
    jle    L1         ; if (n<=1) goto L1  
    push   rbp  
    mov    rbp, rsp  
    mov    rcx, rdi  
    sub    rcx, 1  
    mov    rdi, rcx  
    call   fib        ; fib(n-1)  
    mov    r12, rax   ; save result  
    mov    rdi, rbp  
    sub    rdi, 2  
    call   fib        ; fib(n-2)  
    add    rax, r12   ; rax = fib(n-1) + fib(n-2)  
    pop    rbp  
    ret  
L1:  
    mov    rax, rdi  ; return n  
    ret  
```  
This outline shows the native stack/frame operations. The TS-native compiler would similarly generate branching and calls, relying on the OS ABI (on Linux SysV: first arg in `rdi`, return in `rax`).  

# References  
- StationX: *scriptc: TypeScript to Native Binaries, No Node Required* (analysis of Vercel’s TS-native compiler).  
- Remojansen, *“I tried to compile TypeScript into a native binary with scriptc”* (Dev.to).  
- Perry Native TS Compiler documentation (SWC+LLVM pipeline, supported features).  
- AssemblyScript Book – *Concepts* (static TS subset, no `any`/`undefined`).  
- Reproducible Builds – *Deterministic Build Systems* (guidelines for stable inputs/outputs).  
- Crystal Lang docs – *Static Linking* (musl vs glibc, OS static support).  
- TypeGPU documentation – *Type-safe WebGPU toolkit* (writing WGSL shaders from TypeScript).  
- Google ADK – *Gemma models for TypeScript agents* (AI agent framework in TS).  
- MDN/WebGPU – *WebGPU API overview* (W3C GPU standard).  
- Farouk Elalem, *“Under the Hood of Solana Program Execution”* (Mermaid diagrams of the compile pipeline).  
- **Microsoft Dev Digest:** *TypeScript native previews (Project Corsa, TS7)* – Go-based TS compiler ~10× faster.  

