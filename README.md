# epa

A confined generation tool: it runs an LLM over a local directory, read-only, and emits
the model's output as a usable product that is always labeled untrusted. One binary is a
one-shot CLI (`epa`); an optional second (`epa-mcp`) serves the same generation core as an
MCP tool over stdio. Confinement, resource caps, the read-only tool set, and the
model/tool loop come from [myelin](https://github.com/fruitvibes123/myelin); an optional
feature links the [creatine](https://github.com/fruitvibes123/creatine) inference engine
in process.

> [!WARNING]
> Every line of code and documentation in this repository is LLM output, written end to end by
> Claude (Anthropic) under the direction and review of a human operator. Read and reuse it with
> that provenance in mind.

This repository is a code-review export: the sources carry no comments, and this file is
the crate's documentation. See "About this export" below.

## The output contract

epa's product is generated text meant to be used, so epa does not redact it. Its safety
rests on confined inputs, bounded compute, no agency, and an output that always says what
it is.

- Every result of a run (success, cap exit, transport fault) carries an
  `UntrustedGenerated` marker and factual provenance: the model name, the scope globs, the
  files opened, and what (if anything) truncated the run. A refusal before a run starts is
  different: it exits 2 with a plain `epa: <message>` diagnostic and no marked envelope,
  and on `--json` it writes nothing to stdout. The library API also exposes the iteration,
  tool-call and wall-clock counts and a stop reason via `MarkedResult::meta()`; the CLI
  and MCP envelopes do not carry them. epa reports facts and does not certify its own
  output. An incomplete scan's silence would be false assurance, so epa has no output
  safety-scanner. The caller validates before use.
- Human mode writes the marker line and the provenance line to stderr. In confined-read
  mode, when a failure cause was recorded (a backend or transport error), a `[epa]
  cause=…` line naming it also goes to stderr. The product goes to stdout. A productless
  result with no recorded cause (an empty model reply, an output-budget or iteration cap),
  and any productless result in inline mode, carries the marker and provenance but no
  cause line; the `truncation` field distinguishes those cases. The block is one gated
  unit: epa writes the whole block to stderr first and writes the product to stdout only
  after that succeeds, so a truncated product never ships without its truncation label.
  Model output is untrusted, so when stdout is a terminal the product is
  control-byte-escaped (see "Terminal output safety"); when stdout is a pipe or file it is
  byte-exact.
- `--json` writes one stdout object carrying the marker, the product, and a provenance
  sub-object (model, scope globs, files opened, and truncation), so they cannot be
  separated on the single stream. The object is pure ASCII: DEL and every byte ≥ 0x80 are
  escaped to round-tripping `\u` form, so no raw C1 introducer reaches the output.
  `provenance.truncation` is one of `none`, `gen_length`, `budget`, `backend_budget`,
  `no_product`.
- Exit codes: `0` the product bytes were written to stdout, `1` ran but product-less (or
  the product write failed), `2` refused before running.

## Modes

- Confined-read (the default): the tool loop reads inside a confined root through myelin's
  read-only tools (`read_file`, `grep`, `list_dir`, optionally `git_log` / `git_diff`);
  the model's first tool-less turn is the product. On a cap exit one forced tool-less turn
  is added instead (see Limitations). The toolbox holds no write, exec or network tool. It
  is constructed without a search backend, and a `search` config key is an unknown field
  that refuses at parse, so the config cannot ask for one. The optional git tools run the
  real `git` binary (see Limitations).
- Inline (`"mode": "inline"`): a single model turn with no read surface; the instruction
  carries the entire input. A config that combines inline mode with any read-surface key
  (`root`, `default_scope`, `git`, …) refuses to start, as does `--root` or a per-call
  `--scope` against an inline config.

## Backends

- Connection (`model_endpoint`): an OpenAI-wire HTTP endpoint over TCP loopback, a Unix
  socket, or TLS with SPKI key pinning (`model_pin`), via myelin's client. This is the
  shipped default build's only backend.
- In-process (`model_path`, feature `inprocess-model`): loads a GGUF model through
  creatine's family factory and runs the engine in this process, with no socket and no
  HTTP parse surface. Linking the engine in process brings its input-parse surfaces (GGUF,
  tokenizer) into the epa process, where the connection backend keeps them in a separate
  process, so this backend is off by default and an explicit deployment opt-in. Each
  generation job gets a fresh non-content session key, so the engine's session-keyed cache
  never crosses jobs. On a build without the engine, an in-process config is a typed
  refusal and nothing falls back.
- Host-owned engines: `run_with_engine` (CLI core) and `with_owned_engine` (MCP) let a
  host that already owns a creatine engine inject it; epa still owns all confinement and
  the marked emit. The injected engine is the only engine, so its config must declare no
  backend: a config carrying `model_endpoint` or `model_path` on these seams is refused as
  contradictory.

For the connection and in-process backends, exactly one must be configured, and a pin
without a connection endpoint is refused. On the host-owned-engine seams the rule is zero,
per the bullet above.

## The startup gate

The config is one owner-only JSON file (`--config PATH`), parsed with unknown fields
refused and validated fail-closed before anything runs. "Owner-only" is a permission-bit
check: the group and other bits must all be clear (mode `& 0o077 == 0`). It does not check
the file's owner. The check applies to the file the path resolves to; a `--config` symlink
is followed and its target is checked.

Disclosure: the `sensitive` and `tier_b` booleans classify the deployment. Either one
absent or true makes it strict. Strict requires a non-empty `default_scope` and `git:
false`, and refuses per-call `--scope` on any surface that is not explicitly trusted
on-box (loopback, unix socket, or in-process). The only non-strict config sets both flags
to false explicitly, which states that the root holds nothing sensitive and no second tier
is involved.

A scope list is bounded before it is compiled: at most 256 globs, each at most 1024 bytes.
An over-limit list refuses (CLI exit 2, MCP `-32602`) before any glob is built. The bound
is checked per call, when a scope is built, and not at startup: an over-limit
`default_scope` starts the server and is refused on the first call that builds its scope
(the server stays up and answers with the `-32602`). The bound applies to a per-call
`--scope` / `scope_globs` list and to the config `default_scope` alike.

The remaining keys, with defaults:

- `root`: the confined read root. No default; a root given by neither the config nor
  `--root` is a startup refusal.
- `git`: default `false`; off unless written `true`.
- `excluded_dirs`: default `["target","node_modules","dist","build"]`, the directories the
  `grep` walk skips. `read_file` and `list_dir` do not consult it, so it filters the grep
  walk only and is not a read boundary; a directory that must not be read needs
  `default_scope`.
- `bounds`: myelin's per-tool byte/entry/deadline bounds.
- `caps`: per-call iteration / tool-call / output / wall-clock caps.
- `max_gen_tokens`: default `4096`.
- `max_session_output_bytes`: default 1 MiB.
- `model`: default `"local-model"`, sent as the model name to the wire endpoint.
- `model_timeout_secs`: default `300`.

`max_gen_tokens` reaches the engine through creatine's request caps, which a connection
build does not construct, so it applies to in-process builds only. The connection backend,
the shipped default build's only backend, sends no token limit on the wire. On a
connection build the generation bounds that apply are `max_session_output_bytes`, myelin's
client response-body limit, and `caps.wall_clock_ms`.

## The MCP server

`epa-mcp` (feature `mcp`) serves one tool, `local_generate`, over stdio. stdout belongs to
the MCP transport, and every diagnostic epa constructs is sanitized before it reaches
stderr. The session output budget is server-lifetime and shared across calls behind a
mutex, so the aggregate bound spans the server and calls serialize: one slow or hung
`local_generate` call stalls subsequent calls on that server.

The loop checks the wall clock once per iteration, at the start. After that check it runs
one model call and then every tool dispatch the model returned in that response, and none
of that post-check work (the model call, the tool dispatches, their per-tool `ToolBounds`
deadlines) is wall-clock checked. The first cap to fire ends the loop. When a cap other
than the wall clock fires after at least one tool dispatch and with no draft, the loop may
add one post-cap wrap-up model call (see Limitations). A single `local_generate` call
therefore runs for roughly `caps.wall_clock_ms` (or until the first cap fires), plus the
model call and tool dispatches of the iteration in flight when it fired, plus the wrap-up
model call on a cap stop; no post-check term is wall-clock bounded. On a connection build
each model call is also bounded by `model_timeout_secs`; on an in-process build
`model_timeout_secs` is not read on the generation path. No single timeout knob bounds a
call, so a client must not size its own request timeout on the assumption of a tight
bound. The deployment bounds the tail by lowering `max_tool_calls` (default `96`), the
per-tool deadlines (the `git` and `grep` deadlines default to `30_000` ms each), and
`caps.wall_clock_ms`. Compiling the call's scope globs is a pre-loop term a caller
controls; the fixed scope caps above (256 globs, 1024 bytes each) bound it, so it stays
short whatever these settings are.

`epa-mcp` runs an in-flight generation to completion, but it delivers a reply only while
the client keeps stdin open. If the client closes stdin before reading the reply of an
in-flight generation, the reply may be dropped after the transport's post-EOF drain window
(about 5 seconds), and the process still exits `0`. A client that needs the reply keeps
stdin open until it has read the response; the canonical MCP client holds stdin open for
the session's life. With `mcp-inprocess` the server owns a creatine engine for its
lifetime and mints a fresh borrowing adapter and session key per call. `epa-mcp` exits `0`
on a clean shutdown and `1` on any refusal or fault before or during serving.

## Terminal output safety

epa treats its own terminal as an output sink that untrusted bytes must not program
(CWE-150). Every diagnostic epa writes, and the product when stdout is a terminal, passes
through one sanitizer that escapes:

- the C0/C1 controls except LF and TAB (neither of those two can program a terminal, and
  keeping them preserves the product's line structure);
- the full Unicode `Cf` (invisible-format) category: bidi overrides, zero-widths, soft
  hyphen, the prepended-concatenation and shaping marks, interlinear annotation, the tag
  block;
- the line and paragraph separators (U+2028 / U+2029).

The escaped set is exactly the Unicode `Cf` category plus U+2028 and U+2029; an outsider
can recompute the category from the Unicode data and check the table in `src/cli.rs`
against it. The `--json` surface is the machine-parse path and is pure ASCII by
construction.

This escaping covers the diagnostics and product epa constructs. The MCP transport layer
(rmcp) is separate. For a malformed request (an unknown parameter name, or an unknown
JSON-RPC method), rmcp itself constructs the protocol-level JSON-RPC error, before or
around epa's handler, and that error echoes the caller-supplied name verbatim. Owning that
serialization would mean forking rmcp or dropping the unknown-argument refusal, so epa
does not sanitize those bytes. A client that renders an rmcp protocol error straight to a
terminal owns that escaping. epa's own tool-error replies and its success envelope on the
same stream are pure ASCII.

## Feature graph and sovereignty

The default graph carries myelin (with its HTTP/TLS surface: ureq + rustls/ring) plus
serde/serde_json/thiserror/globset, and no creatine, tokio or rmcp. Features:

- `creatine-inprocess`: the engine adapter seam (creatine lib core only, no model
  execution backend).
- `inprocess-model`: the real in-process engine (pulls creatine's `qwen2` stack: candle's
  tensor core and the tokenizer stack, including its statically-linked oniguruma C
  library).
- `mcp`: the stdio MCP server (rmcp + tokio, io only: no hyper, no TLS stack of its own).
- `mcp-inprocess`: both.

No openssl, aws-lc, native-tls, or cudarc crate is in any configuration's resolved graph.
The gate is a grep over the full tree that must return nothing:

```
cargo tree --features mcp-inprocess --edges normal --prefix none --format '{p}' | sort -u | grep -Ei 'openssl|aws-lc|native-tls|cudarc'
```

## Building and testing

Rust, edition 2024. `rust-version` is declared as 1.89, and cargo enforces that floor for
every configuration, so the five `cargo test --locked` commands below all require 1.89.
The underlying split is finer than a single manifest floor can express: the three
connection-only configurations (default, `creatine-inprocess`, `mcp`) compile on rustc
1.88 only with `--ignore-rust-version`. creatine's SIMD feature detection needs 1.89, so
the two in-process configurations (`inprocess-model`, `mcp-inprocess`) need it even with
that flag. Five configurations:

```
cargo test --locked
cargo test --locked --features creatine-inprocess
cargo test --locked --features inprocess-model
cargo test --locked --features mcp
cargo test --locked --features mcp-inprocess
```

The confined-read boundary is `openat2(2)` with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS |
RESOLVE_NO_XDEV | RESOLVE_NO_MAGICLINKS`, a Linux 5.6 syscall (all four `RESOLVE_*` flags
landed together in 5.6). A kernel without the syscall returns `ENOSYS`, which epa maps to
a refusal (there is no unsafe-resolution fallback), so every confined read fails closed on
such a kernel, and Linux 5.6 or newer is the runtime floor for any read.

`cargo clippy --all-targets -- -D warnings` in each configuration is part of the gate; the
no-panic bar on the request path is a set of deny-level clippy lints re-declared in each
bin. The myelin and creatine dependencies are pinned git revisions fetched from GitHub. A
few end-to-end tests load a real model and are gated behind `EPA_MODEL_GGUF` (a path to a
GGUF file); without it they skip.

## Limitations

- Scope semantics are myelin's. The glob dialect crosses `/`, and a scope with a wildcard
  segment leaves directory names and existence visible to `list_dir` while file names and
  contents stay filtered; myelin's README has the full statements.
- Git tools need a trusted repository. Git tooling is off unless `git: true` is written;
  an absent `git` key is off. `git: true` (non-strict configs only) runs the real `git`
  binary with myelin's argv and environment hardening, but repo-local `.git/config` can
  still name programs git executes. Point epa's git tools only at a repository whose
  config you trust. Git tools are also coupled to the scope. Scope globs do not apply to
  git, so myelin refuses every git dispatch whenever `default_scope` is non-empty; `git:
  true` offers `git_log`/`git_diff` to the model, but a git dispatch runs only in a
  configuration with an empty `default_scope`, that is, with no read filter.
- A hardlink can reach outside the root. A hardlink placed inside the root to a file
  outside it reads the outside file's content, and no path-resolution confinement
  distinguishes it from an ordinary in-root file. Creating one requires write access to
  the root, which epa's confined-read model grants to nothing it runs.
- The connection client bounds response bodies. A model product larger than the client's
  body bound comes back with truncation `no_product` (the transport-fault label); the run
  reports product-less, and no partial product is emitted.
- A per-call `--scope` replaces the configured `default_scope` and is never intersected
  with it. It is accepted only on an explicitly-trusted on-box surface and refused
  elsewhere; a scope-intersect for restricted surfaces is not built.
- The MCP server serializes calls (see above); it is a single-session surface.
- Inline mode has nothing to confine. The startup gate refuses any config that combines
  inline mode with a read surface.
- The post-cap wrap-up call runs past the cap (myelin's loop behavior). When a
  non-wall-clock cap fires after at least one tool dispatch with no draft, one final
  tool-less model call runs. On a connection build that call is bounded by
  `model_timeout_secs`; on an in-process build `model_timeout_secs` is not read on the
  generation path, so the bound on that call is `caps.wall_clock`.

## License

epa is licensed GPL-2.0-or-later; the text is in COPYING.

myelin is MIT OR Apache-2.0. creatine, linked only under the in-process features, is
GPL-2.0-or-later. The remaining dependencies carry their own permissive licenses
(Apache-2.0 and MIT dominate the graph).

## About this export

This is a published export of an internal tree. In the Rust sources, every comment, doc
comments included, is blanked in place: the comment's characters become whitespace and the
code is left untouched, so line numbers map to the internal tree and the code on each line
is byte-identical. A doc-comment line keeps its character count and a line comment keeps
its byte count, so byte offsets are not preserved: an offset past a non-ASCII comment
drifts from the internal tree. `cargo fmt --check` reports a diff that is whitespace-only
except for one enum-variant re-wrap: the blank-line run left where comments were blanked
makes rustfmt re-emit one variant in expanded form with a trailing comma, a syntactic
no-op (`cargo fmt` rewrites it; the suite stays green). `Cargo.toml` is scrubbed by a
different rule: its comment lines are emptied of content, so line numbers still map to the
internal tree but byte offsets do not. The gates that hold on this tree are `cargo build`,
`cargo clippy`, and `cargo test` across the five configurations above.
