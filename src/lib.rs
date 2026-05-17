// SPDX-License-Identifier: MIT
//! Generic build-helper primitives for proto-source services.
//!
//! The four primitives this crate exposes are the ones every
//! `.proto`-publishing or `.proto`-consuming Rust crate ends up
//! reimplementing:
//!
//! 1. **Stage embedded proto bytes onto a tempdir** at protoc-relative
//!    paths so `import "myproto/v1/foo.proto";` resolves at build time
//!    without the consumer vendoring the file ([`Stager`]).
//! 2. **Compile `.proto` files via `protox`** (pure Rust, no `protoc`
//!    subprocess) and return both the `prost-reflect` descriptor pool
//!    (preserves custom-option VALUES) and the FDS bytes for downstream
//!    codegen ([`compile_protos`]).
//! 3. **Read `MethodOptions` extension values** from the descriptor pool
//!    — encoded FDS drops them, the pool keeps them
//!    ([`extract_method_string_extension`]).
//! 4. **Drive `tonic-prost-build`** with `type_attribute(...)` injection
//!    from an annotation-FQN map ([`tonic_prost_build_with_attrs`], gated
//!    on the `tonic` feature).
//!
//! The crate is **schema-agnostic** — it doesn't know about any specific
//! proto package or annotation. Consumers pair it with a tiny sibling
//! crate that ships their `.proto` bytes via `include_bytes!`:
//!
//! ```ignore
//! // some-protos/src/lib.rs (your bytes crate):
//! const FOO: &[u8] = include_bytes!("../proto/my/v1/foo.proto");
//! pub fn files() -> impl Iterator<Item = (&'static str, &'static [u8])> {
//!     [("my/v1/foo.proto", FOO)].into_iter()
//! }
//! ```
//!
//! Consumer `build.rs` then composes any number of `*-protos` crates:
//!
//! ```ignore
//! use proto_build_kit::Stager;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let staged = Stager::new()
//!         .with(some_protos::files())
//!         .with(other_protos::files())
//!         .stage()?;
//!
//!     // Drive whatever codegen you want (connectrpc-build, tonic, ...)
//!     // against `staged.path()` on the include path.
//!     Ok(())
//! }
//! ```
//!
//! Hold the returned [`tempfile::TempDir`] until codegen completes —
//! drop deletes the staged files.

#![doc(html_root_url = "https://docs.rs/proto-build-kit/0.1.0")]

mod compile;
mod errors;
mod extract;
mod stage;

#[cfg(feature = "tonic")]
mod codegen_tonic;

pub use compile::{CompileOutput, compile_protos};
pub use errors::Error;
pub use extract::extract_method_string_extension;
pub use stage::Stager;

#[cfg(feature = "tonic")]
pub use codegen_tonic::tonic_prost_build_with_attrs;
