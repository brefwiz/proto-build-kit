# proto-build-kit

[![crates.io](https://img.shields.io/crates/v/proto-build-kit.svg)](https://crates.io/crates/proto-build-kit)
[![docs.rs](https://img.shields.io/docsrs/proto-build-kit)](https://docs.rs/proto-build-kit)

Generic build-helper primitives for proto-source services in Rust.

## What

Four primitives every `.proto`-publishing or `.proto`-consuming Rust
crate ends up reimplementing:

1. **Stage embedded proto bytes** onto a tempdir at protoc-relative
   paths so `import "myproto/v1/foo.proto";` resolves at build time
   without the consumer vendoring the file (`Stager`).
2. **Compile `.proto` files via `protox`** (pure Rust, no `protoc`
   subprocess) returning both the `prost-reflect` descriptor pool and
   FDS bytes (`compile_protos`).
3. **Read `MethodOptions` extension values** from the descriptor pool —
   encoded FDS drops them, the pool keeps them
   (`extract_method_string_extension`).
4. **Drive `tonic-prost-build`** with `type_attribute(...)` injection
   from an annotation→attribute map (`tonic_prost_build_with_attrs`,
   behind the `tonic` feature).

Schema-agnostic: the crate knows nothing about any specific proto
package or custom option. Pair it with a sibling `*-protos` crate that
exposes raw bytes.

## Pattern

A proto-publishing repo ships a tiny `*-protos` crate that embeds its
proto bytes:

```rust
// some-protos/src/lib.rs (~10 lines)
const FOO: &[u8] = include_bytes!("../proto/foo/v1/foo.proto");
const BAR: &[u8] = include_bytes!("../proto/foo/v1/bar.proto");

pub fn files() -> impl Iterator<Item = (&'static str, &'static [u8])> {
    [
        ("foo/v1/foo.proto", FOO),
        ("foo/v1/bar.proto", BAR),
    ]
    .into_iter()
}
```

The `*-protos` crate has **zero dependencies** on `proto-build-kit`.
It just exposes the bytes. Consumer `build.rs` composes any number of
them:

```rust
use proto_build_kit::Stager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let staged = Stager::new()
        .with(some_protos::files())
        .with(other_protos::files())
        .stage()?;

    // Then drive whatever codegen you want (connectrpc-build, tonic, ...)
    connectrpc_build::Config::new()
        .files(&["proto/my/v1/svc.proto"])
        .includes(&["proto/", staged.path()])
        .include_file("_connectrpc.rs")
        .compile()?;
    Ok(())
}
```

Hold the returned `TempDir` until codegen completes — drop deletes
the staged files.

## Annotation-driven codegen

For services that drive type derives from custom proto annotations:

```rust
use proto_build_kit::{compile_protos, extract_method_string_extension,
                      tonic_prost_build_with_attrs, Stager};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let staged = Stager::new()
        .with(some_protos::files())                // canonical shapes
        .with(my_conventions_protos::files())       // your annotation file
        .stage()?;

    let out = compile_protos(
        &["proto/my/v1/svc.proto"],
        &["proto", staged.path()],
    )?;

    // Walk methods, find `(my.conventions.etag_field) = "version"` options.
    let etag_fields =
        extract_method_string_extension(&out.pool, "my.conventions.etag_field");

    // Build a type_attribute map: per response-type FQN, derives to inject.
    let mut attrs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (response_fqn, etag) in etag_fields {
        attrs.entry(response_fqn).or_default().push(format!(
            "#[derive(::my_crate::Envelope)] #[envelope(etag = \"{etag}\")]"
        ));
    }

    tonic_prost_build_with_attrs(&out.fds_bytes, &attrs, |b| {
        b.build_server(true).build_client(true)
    })?;
    Ok(())
}
```

## Features

- `tonic` (default) — enables `tonic_prost_build_with_attrs`. Disable
  if you drive your own codegen (`connectrpc-build`, custom) and only
  need stage / compile / extract.

## License

MIT. See [LICENSE](./LICENSE).
