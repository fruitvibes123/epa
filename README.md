# epa

A confined generation tool: it runs an LLM over a local directory, read-only, and emits
the model's output as a usable product that is always labeled untrusted. One binary is a
one-shot CLI (`epa`), an optional second (`epa-mcp`) serves the same generation core as an
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

epa's product is generated text meant to be used, so its safety story is not redaction.
It is: confined inputs, bounded compute, no agency, and an output that always says what
it is.

- Every result of a run — success, cap exit, transport fault — carries an
  `UntrustedGenerated` marker and factual provenance: the model name, the scope globs,
  the files opened, and what (if anything) truncated the run. A refusal before a run
  starts is different: it exits 2 with a plain `epa: <message>` diagnostic and no marked
  envelope, and on `--json` it writes nothing to stdout. The library API additionally
  exposes the iteration, tool-call and wall-clock counts and a stop reason via
  `MarkedResult::meta()`; the CLI and MCP envelopes do not carry them. Facts, never
  verdicts: epa does not certify its own output, and there is deliberately no output
  safety-scanner (an incomplete scan's silence would be false assurance). The caller
  validates before use.
- Human mode: the marker line, the provenance line, and — in confined-read mode, when a
  failure cause was recorded (a backend or transport error) — a `[epa] cause=…` line naming
  it go to stderr; the product goes to stdout. A productless result with no recorded cause
  (an empty model reply, an output-budget or iteration cap), and any productless result in
  inline mode, carries the marker and provenance but no cause line; the `truncation` field
  distinguishes those cases. The block is one gated unit: the
  product is written to stdout only if the whole block first reached stderr, so a truncated
  product never ships without its truncation label. When stdout is a terminal the product is
  control-byte-escaped (model output is untrusted; see "Terminal output safety"); when
  stdout is a pipe or file it is byte-exact.
- `--json`: one stdout object carrying the marker, the product, and a provenance sub-object
  (model, scope globs, files opened, and truncation) — inseparable on a single stream,
  emitted as pure ASCII (DEL and every byte ≥ 0x80 escaped to round-tripping `\u` form) so
  no raw C1 introducer rides the wire. `provenance.truncation` is one of `none`, `gen_length`,
  `budget`, `backend_budget`, `no_product`.
- Exit codes: `0` the product bytes were written to stdout, `1` ran but product-less (or
  the product write failed), `2` refused before running.

## Modes

- **Confined-read** (the default): the tool loop reads inside a confined root through
  myelin's read-only tools (`read_file`, `grep`, `list_dir`, optionally `git_log` /
  `git_diff`); the model's first tool-less turn is the product. On a cap exit one forced
  tool-less turn is added instead (see Limitations). There is no write tool,
  no exec tool, and no network tool: the toolbox is constructed without a search backend,
  and the config cannot ask for one — a `search` key is an unknown field and refuses at
  parse. The optional git tools run the real `git` binary (see Limitations).
- **Inline** (`"mode": "inline"`): a single model turn with no read surface at all — the
  instruction carries the entire input. A config that combines inline mode with any
  read-surface key (`root`, `default_scope`, `git`, …) refuses to start, as does `--root`
  or a per-call `--scope` against an inline config.

## Backends

- **Connection** (`model_endpoint`): an OpenAI-wire HTTP endpoint over TCP loopback, a
  Unix socket, or TLS with SPKI key pinning (`model_pin`), via myelin's client. This is
  the shipped default build's only backend.
- **In-process** (`model_path`, feature `inprocess-model`): loads a GGUF model through
  creatine's family factory and runs the engine in this process — no socket, no HTTP
  parse surface. Off by default and an explicit deployment opt-in: linking the engine
  in-process shares its input-parse surfaces (GGUF, tokenizer) into the epa process,
  where the connection backend keeps them in a separate process. Each generation job gets
  a fresh non-content session key, so the engine's session-keyed cache never crosses
  jobs. On a build without the engine, an in-process config is a typed refusal, never a
  silent fallback.
- **Host-owned engines**: `run_with_engine` (CLI core) and `with_owned_engine` (MCP) let
  a host that already owns a creatine engine inject it; epa still owns all confinement
  and the marked emit. The injected engine is the only engine, so its config must declare
  no backend: a config carrying `model_endpoint` or `model_path` on these seams is refused
  as contradictory.

For the connection and in-process backends, exactly one must be configured, and a pin
without a connection endpoint is refused. On the host-owned-engine seams the rule is
zero, per the bullet above.

## The startup gate

The config is one owner-only JSON file (`--config PATH`), parsed with unknown fields
refused, validated fail-closed before anything runs. "Owner-only" is a permission-bit
check — the group and other bits must all be clear (mode `& 0o077 == 0`); it does not
check the file's owner. The check is applied to the file the path resolves to: a
`--config` symlink is followed and its target is checked.

Disclosure: the `sensitive` and `tier_b` booleans classify the deployment. Either one
absent or true makes it strict; strict requires a non-empty `default_scope` and
`git: false`, and refuses per-call `--scope` on any surface that is not explicitly
trusted on-box (loopback, unix socket, or in-process). The only non-strict config is both
flags explicitly false — an explicit statement that the root holds nothing sensitive and
no second tier is involved.

A scope list is bounded before it is compiled: at most 256 globs, each at most 1024 bytes.
An over-limit list refuses (CLI exit 2, MCP `-32602`) before any glob is built. This is a
per-call bound checked when a scope is built, not a startup check: an over-limit
`default_scope` starts the server and is refused on the first call that builds its scope
(the server stays up and answers with the `-32602`). The bound applies to a per-call
`--scope` / `scope_globs` list and to the config `default_scope` alike.

The remaining keys, with defaults: `root` (the confined read root; no default — an absent
root, from neither the config nor `--root`, is a startup refusal); `git` (default
`false` — off unless written `true`); `excluded_dirs` (default
`["target","node_modules","dist","build"]` — the directories the `grep` walk skips; it is
a grep-walk filter only, not a read boundary: `read_file` and `list_dir` do not consult
it, so a directory that must not be read needs `default_scope`, not `excluded_dirs`);
`bounds` (myelin's per-tool
byte/entry/deadline bounds); `caps` (per-call iteration / tool-call / output / wall-clock
caps); `max_gen_tokens` (default `4096`); `max_session_output_bytes` (default
1 MiB); `model` (default `"local-model"` — sent as the model name to the wire endpoint);
`model_timeout_secs` (default `300`).

`max_gen_tokens` applies to in-process builds only — it reaches the engine through
creatine's request caps, which a connection build does not construct. The connection
backend, the shipped default build's only backend, sends no token limit on the wire; on a
connection build the generation bounds that do apply are `max_session_output_bytes`,
myelin's client response-body limit, and `caps.wall_clock_ms`.

## The MCP server

`epa-mcp` (feature `mcp`) serves one tool, `local_generate`, over stdio; stdout belongs
to the MCP transport and every diagnostic epa constructs is sanitized before it reaches
stderr. The session output budget is server-lifetime and shared across calls behind a
mutex, so the aggregate bound spans the server — and calls serialize: one slow or hung
`local_generate` call stalls subsequent calls on that server. A single call is not bounded
by any one timeout knob. The loop checks the wall clock once per iteration, at the start;
after that check it runs one model call and then every tool dispatch the model returned in
that response, and none of that post-check work — neither the model call, nor the tool
dispatches, nor their per-tool `ToolBounds` deadlines — is wall-clock checked. Whichever
cap fires first ends the loop, and when a cap other than the wall clock fires the loop may
add one post-cap wrap-up model call on top (only when that stop dispatched at least one tool
and has produced no draft — see Limitations). So a single `local_generate` call runs for roughly
`caps.wall_clock_ms` (or whichever cap fires first) plus the model call and tool dispatches
of the iteration in flight when it fired plus, on a cap stop, the wrap-up model call — and
no post-check term is wall-clock bounded. On a connection build each model call is
additionally bounded by `model_timeout_secs`; on an in-process build `model_timeout_secs`
is not read on the generation path. A client must not size its own request timeout assuming
a tight bound. The deployment bounds the tail by lowering `max_tool_calls` (default `96`),
the per-tool deadlines (the `git` and `grep` deadlines default to `30_000` ms each), and
`caps.wall_clock_ms`. Compiling the call's scope globs is a pre-loop term a caller
controls; the fixed scope caps above (256 globs, 1024 bytes each) bound it, so it stays
short regardless of these levers. `epa-mcp` runs an in-flight generation to completion, but
a reply is only delivered while the client keeps stdin open: an in-flight generation whose
reply is not read before the client closes stdin may be dropped after the transport's
post-EOF drain window (about 5 seconds) with the process still exiting `0`. A client that
needs the reply keeps stdin open until it has read the response; the canonical MCP client
holds stdin open for the session's life. With `mcp-inprocess` the server owns a creatine
engine for its lifetime and mints a fresh borrowing adapter and session key per call.
`epa-mcp` exits `0` on a clean shutdown and `1` on any refusal or fault before or during
serving.

## Terminal output safety

epa treats its own terminal as an output sink that untrusted bytes must not program
(CWE-150). Every diagnostic epa writes, and the product when stdout is a terminal, passes
through one sanitizer that escapes the C0/C1 controls (except LF and TAB, which are kept so
the product's line structure survives and neither can program a terminal), the full
Unicode `Cf` (invisible-format) category — bidi overrides, zero-widths, soft hyphen, the
prepended-concatenation and shaping marks, interlinear annotation, the tag block — and
the line and paragraph separators (U+2028 / U+2029). The escaped set is exactly the Unicode
`Cf` category plus U+2028 and U+2029; an outsider can recompute the category from the
Unicode data and check the table in `src/cli.rs` against it.
The `--json` surface is the machine-parse path and is pure ASCII by construction instead.

This escaping covers the diagnostics and product epa constructs. The MCP transport layer
(rmcp) is separate: for a malformed request — an unknown parameter name, or an unknown
JSON-RPC method — rmcp itself constructs the protocol-level JSON-RPC error, before or
around epa's handler, and that error echoes the caller-supplied name verbatim. Those bytes
are not epa-sanitized (epa cannot own that serialization without forking rmcp or dropping
the unknown-argument refusal), so a client that renders an rmcp protocol error straight to
a terminal owns that escaping. epa's own tool-error replies and its success envelope on the
same stream are pure ASCII.

## Feature graph and sovereignty

The default graph carries myelin (with its HTTP/TLS surface: ureq + rustls/ring) plus
serde/serde_json/thiserror/globset — no creatine, no tokio, no rmcp. Features:

- `creatine-inprocess` — the engine adapter seam (creatine lib core only, no model
  execution backend).
- `inprocess-model` — the real in-process engine (pulls creatine's `qwen2` stack:
  candle's tensor core, the tokenizer stack including its statically-linked oniguruma C
  library).
- `mcp` — the stdio MCP server (rmcp + tokio, io only: no hyper, no TLS stack of its
  own).
- `mcp-inprocess` — both.

No openssl, aws-lc, native-tls, or cudarc crate is in any configuration's resolved
graph. The gate is a grep over the full tree that must return nothing:

```
cargo tree --features mcp-inprocess --edges normal --prefix none --format '{p}' | sort -u | grep -Ei 'openssl|aws-lc|native-tls|cudarc'
```

## Building and testing

Rust, edition 2024; `rust-version` is declared as 1.89, and cargo enforces that floor for
every configuration — the five `cargo test --locked` commands below all require 1.89. The
underlying split is finer than a single manifest floor can express: the three
connection-only configurations (default, `creatine-inprocess`, `mcp`) compile on rustc 1.88
only with `--ignore-rust-version`; the two in-process configurations (`inprocess-model`,
`mcp-inprocess`) need 1.89 for creatine's SIMD feature detection even with that flag. Five
configurations:

```
cargo test --locked
cargo test --locked --features creatine-inprocess
cargo test --locked --features inprocess-model
cargo test --locked --features mcp
cargo test --locked --features mcp-inprocess
```

The confined-read boundary is `openat2(2)` with
`RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_XDEV | RESOLVE_NO_MAGICLINKS`, a Linux
5.6 syscall (all four `RESOLVE_*` flags landed together in 5.6). Linux 5.6 or newer is the
runtime floor for any read: a kernel without the syscall returns `ENOSYS`, which epa maps to
a refusal (there is no unsafe-resolution fallback), so every confined read fails closed on
such a kernel.

`cargo clippy --all-targets -- -D warnings` in each configuration is part of the gate;
the no-panic bar on the request path is a set of deny-level clippy lints re-declared in
each bin. The myelin and creatine dependencies are pinned git revisions fetched from
GitHub. A few end-to-end tests load a real model and are gated behind `EPA_MODEL_GGUF`
(a path to a GGUF file); without it they skip.

## Limitations

- **Scope semantics are myelin's.** The glob dialect crosses `/`, and a scope with a
  wildcard segment leaves directory names and existence visible to `list_dir` while file
  names and contents stay filtered — see myelin's README for the full statements.
- **Git tools mean a trusted repository.** Git tooling is off unless `git: true` is
  written; an absent `git` key is off. `git: true` (non-strict configs only) runs the
  real `git` binary with myelin's argv and environment hardening, but repo-local
  `.git/config` can still name programs git executes. Point epa's git tools only at a
  repository whose config you trust. Git tools are also coupled to the scope: myelin
  refuses every git dispatch whenever `default_scope` is non-empty (scope globs do not
  apply to git), so `git: true` offers `git_log`/`git_diff` to the model but the only
  configuration in which a git dispatch runs is one with an empty `default_scope`, i.e.
  with no read filter.
- **A hardlink can reach outside the root.** A hardlink placed inside the root to a file
  outside it reads the outside file's content, and no path-resolution confinement
  distinguishes it from an ordinary in-root file. Creating one requires write access to
  the root, which epa's confined-read model grants to nothing it runs.
- **The connection client bounds response bodies.** A model product larger than the
  client's body bound comes back with truncation `no_product` (the transport-fault
  label); the run reports product-less, and no partial product is emitted.
- **Per-call `--scope` replaces, never intersects.** It is accepted only on an
  explicitly-trusted on-box surface and refused elsewhere; a scope-intersect for
  restricted surfaces is not built.
- **The MCP server serializes calls** (see above); it is a single-session surface, not a
  concurrent one.
- **Inline mode's confinement is vacuous by design** — there is nothing to confine; the
  gate instead refuses any config that tries to combine inline mode with a read surface.
- **The post-cap wrap-up call runs past the cap** (myelin's loop behavior): when a
  non-wall-clock cap fires after at least one tool dispatch with no draft, one final
  tool-less model call runs. On a connection build that call is bounded by
  `model_timeout_secs`; on an in-process build `model_timeout_secs` is not read on the
  generation path, so the bound on that call is `caps.wall_clock`.

## License

epa is licensed GPL-2.0-or-later — see COPYING.

myelin is MIT OR Apache-2.0. creatine, linked only under the in-process features, is
GPL-2.0-or-later. The remaining dependencies carry their own permissive licenses
(Apache-2.0 and MIT dominate the graph).

## About this export

This is a published export of an internal tree. In the Rust sources, every comment — doc
comments included — is blanked in place: the comment's characters become whitespace and
the code is left untouched, so line numbers map to the internal tree and the code on each
line is byte-identical. Byte offsets are not preserved: a doc-comment line keeps its
character count and a line comment keeps its byte count, so an offset past a non-ASCII
comment drifts from the internal tree. `cargo fmt --check` reports a diff that is
whitespace-only except for one enum-variant re-wrap: the blank-line run left where comments
were blanked makes rustfmt re-emit one variant in expanded form with a trailing comma, a
syntactic no-op (`cargo fmt` rewrites it; the suite stays green). The code on each line is
byte-identical to the internal tree regardless. `Cargo.toml` is scrubbed by a different
rule: its comment lines are emptied of content, so line numbers still map to the internal
tree but byte offsets do not. This README is the crate's documentation. The gates that
hold on this tree are `cargo build`, `cargo clippy`, and `cargo test` across the five
configurations above.
