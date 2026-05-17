# proto-build-kit

[![crates.io](https://img.shields.io/crates/v/proto-build-kit.svg)](https://crates.io/crates/proto-build-kit)
[![docs.rs](https://img.shields.io/docsrs/proto-build-kit)](https://docs.rs/proto-build-kit)
[![CI](https://github.com/brefwiz/proto-build-kit/actions/workflows/ci.yml/badge.svg)](https://github.com/brefwiz/proto-build-kit/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**The build-script glue every proto-source Rust project ends up rewriting** — staging embedded `.proto` bytes onto a protoc include path, compiling them via [`protox`](https://crates.io/crates/protox), reading custom `MethodOptions` extension values that `FileDescriptorSet` encoding drops, and driving `tonic-prost-build` with annotation-driven `type_attribute(...)` injection. Schema-agnostic; ~200 lines of focused infrastructure.

## What's in the box

| Primitive | What it does |
|---|---|
| **[`Stager`]** | Accumulate `(relative_path, &'static [u8])` pairs; write them to a fresh tempdir at protoc-relative paths. Duplicate-path detection. The bridge between sibling `*-protos` byte-crates and `protoc`/`buf`/`protox` consumers. |
| **[`compile_protos`]** | `protox` wrapper that returns BOTH the `prost-reflect` descriptor pool (preserves `MethodOptions` extension VALUES — the FDS-encode path drops them) AND the FDS bytes ready for `tonic-build` / `prost-build` / custom codegen. |
| **[`extract_method_string_extension`]** | Walk every method in a descriptor pool, read a string-typed `MethodOptions` extension by FQN, return a map keyed by the response-message FQN. The generic version of "find `(my.envelope.etag_field) = "..."` annotations and tell me which response types declared them." |
| **[`tonic_prost_build_with_attrs`]** | Drive `tonic-prost-build` codegen with a per-type-FQN attribute map (gated on the `tonic` feature, default-on). Inject `#[derive(...)]` and arbitrary `#[my_attr(...)]` from values you extracted via the previous primitive. |

[`Stager`]: https://docs.rs/proto-build-kit/latest/proto_build_kit/struct.Stager.html
[`compile_protos`]: https://docs.rs/proto-build-kit/latest/proto_build_kit/fn.compile_protos.html
[`extract_method_string_extension`]: https://docs.rs/proto-build-kit/latest/proto_build_kit/fn.extract_method_string_extension.html
[`tonic_prost_build_with_attrs`]: https://docs.rs/proto-build-kit/latest/proto_build_kit/fn.tonic_prost_build_with_attrs.html

## Quick start — typical consumer `build.rs`

```rust,no_run
use proto_build_kit::Stager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Stage canonical proto shapes shipped by upstream crates onto a
    // protoc-relative tempdir. Each *-protos crate exposes its bytes
    // through a `files()` iterator (zero dependencies of their own).
    let staged = Stager::new()
        .with(api_bones_protos::files())          // bones/v1/*
        .with(my_internal_protos::files())        // my/internal/v1/*
        .stage()?;

    // Drive your codegen of choice. Here: connectrpc-build.
    connectrpc_build::Config::new()
        .files(&["proto/myservice/v1/svc.proto"])
        .includes(&["proto/", staged.path()])
        .include_file("_connectrpc.rs")
        .compile()?;
    Ok(())
}
```

Hold the returned `tempfile::TempDir` until codegen completes — drop deletes the staged files.

## The `*-protos` crate pattern

A proto-publishing repo ships a tiny sibling crate (typically ~30–80 lines) that embeds its `.proto` bytes and exposes one accessor:

```rust
// myservice-protos/src/lib.rs

const HELLO: &[u8] = include_bytes!("../proto/myservice/v1/hello.proto");
const TYPES: &[u8] = include_bytes!("../proto/myservice/v1/types.proto");

pub fn files() -> impl Iterator<Item = (&'static str, &'static [u8])> {
    [
        ("myservice/v1/hello.proto", HELLO),
        ("myservice/v1/types.proto", TYPES),
    ]
    .into_iter()
}
```

The `*-protos` crate **has zero runtime dependencies** — not even `proto-build-kit`. It just exposes the bytes. Consumers compose any number of `*-protos` crates via `Stager`:

```rust,no_run
# use proto_build_kit::Stager;
let staged = Stager::new()
    .with(a_protos::files())
    .with(b_protos::files())
    .with(c_protos::files())
    .stage()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Stager::stage()` errors loudly on duplicate relative paths — protects against silent shadowing when two `*-protos` crates collide.

## Annotation-driven codegen

For services that derive Rust attributes from custom proto annotations (envelope semantics, validation rules, audit hooks):

```rust,no_run
use proto_build_kit::{compile_protos, extract_method_string_extension,
                      tonic_prost_build_with_attrs, Stager};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let staged = Stager::new()
        .with(my_protos::files())                  // your schema
        .with(my_annotations_protos::files())      // your option declarations
        .stage()?;

    let out = compile_protos(
        &["proto/my/v1/svc.proto"],
        &["proto/", staged.path()],
    )?;

    // For each method declaring `(my.opts.etag_field) = "<field>"`,
    // map the response-message FQN to the field name.
    let etag_fields =
        extract_method_string_extension(&out.pool, "my.opts.etag_field");

    // Build a per-type-FQN attribute injection map.
    let mut attrs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (response_fqn, etag) in etag_fields {
        attrs.entry(response_fqn).or_default().push(format!(
            "#[derive(::my_crate::Envelope)] #[envelope(etag = \"{etag}\")]"
        ));
    }

    // Drive tonic-prost-build with the injection map applied.
    tonic_prost_build_with_attrs(&out.fds_bytes, &attrs, |b| {
        b.build_server(true).build_client(true)
    })?;
    Ok(())
}
```

## Runnable examples

```bash
# Stage two embedded .protos and print compiled descriptor info:
cargo run --example stage_compile

# Extract custom MethodOptions annotations from a fake service:
cargo run --example extract_annotations
```

See `examples/` in the repo for the source.

## Features

- **`tonic`** (default-on) — enables [`tonic_prost_build_with_attrs`]. Disable if you drive your own codegen (`connectrpc-build`, `prost-build`, hand-rolled) and only need stage / compile / extract:

  ```toml
  [build-dependencies]
  proto-build-kit = { version = "0.1", default-features = false }
  ```

## When to use this — and when not to

**Use proto-build-kit when:**

- You publish or consume `.proto` files across multiple Rust crates and want to avoid `git submodule` / hand-vendored `.proto` copies.
- You read custom `MethodOptions` extension values at build time (envelope semantics, validation hints, audit metadata, retry policies).
- You drive `tonic-prost-build` or `connectrpc-build` from a build script and find yourself rewriting the `protox` → descriptor-pool → `type_attribute` plumbing.

**Don't use it when:**

- You're a single-crate proto consumer with one `.proto` file. Just call `tonic-build` directly.
- You don't have custom `MethodOptions` extensions to read at build time.
- You want a buf-CLI-driven workflow. Pair with `buf generate` from a Makefile instead.

## Minimum Supported Rust Version (MSRV)

Rust 1.88+.

## Versioning

Pre-1.0: minor versions may include API breakage with migration notes in [CHANGELOG](CHANGELOG.md). Post-1.0: standard semver.

## License

[MIT](LICENSE).

## Contributing

Issues and PRs welcome. The crate aims to stay small (~200 lines) and unopinionated — new primitives need to clear a "every proto-build script reimplements this" bar before getting added.
