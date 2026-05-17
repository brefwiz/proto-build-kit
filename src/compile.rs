// SPDX-License-Identifier: MIT
//! Compile `.proto` files via `protox` (pure Rust, no `protoc` subprocess).

use std::path::Path;

use prost::Message as _;

use crate::Error;

/// Result of [`compile_protos`].
pub struct CompileOutput {
    /// The in-memory descriptor pool. **Preserves custom-option VALUES**
    /// on `MethodOptions`, which the FDS-encode path drops. Use this
    /// for [`crate::extract_method_string_extension`] and any other
    /// annotation-driven downstream work.
    pub pool: prost_reflect::DescriptorPool,
    /// Encoded `FileDescriptorSet` bytes — suitable for passing to
    /// `tonic_prost_build::Builder::compile_fds(...)` and similar
    /// codegen drivers.
    pub fds_bytes: Vec<u8>,
}

/// Compile a set of `.proto` files (and all their transitive imports)
/// via `protox`.
///
/// `includes` is the protoc-style include path. Pass the path returned
/// by [`crate::Stager::stage()`] plus any caller-provided directories.
/// Well-known types (`google/protobuf/*.proto`) are bundled in `protox`
/// and resolve automatically.
///
/// # Errors
///
/// Returns [`Error::Protox`] if `protox` cannot resolve or parse the
/// inputs.
///
/// # Examples
///
/// ```no_run
/// use proto_build_kit::{compile_protos, Stager};
///
/// fn build() -> Result<(), Box<dyn std::error::Error>> {
///     let staged = Stager::new()
///         .add("my/v1/svc.proto", b"syntax = \"proto3\"; package my.v1;")
///         .stage()?;
///     let out = compile_protos(
///         &["my/v1/svc.proto"],
///         &[staged.path()],
///     )?;
///     assert!(!out.fds_bytes.is_empty());
///     Ok(())
/// }
/// ```
pub fn compile_protos<P: AsRef<Path>, Q: AsRef<Path>>(
    protos: &[P],
    includes: &[Q],
) -> Result<CompileOutput, Error> {
    let mut compiler = protox::Compiler::new(includes.iter().map(AsRef::as_ref))?;
    compiler.include_imports(true);
    compiler.include_source_info(false);
    for p in protos {
        compiler.open_file(p.as_ref())?;
    }

    let pool = compiler.descriptor_pool();
    let fds_bytes = compiler.file_descriptor_set().encode_to_vec();

    Ok(CompileOutput { pool, fds_bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Stager;

    #[test]
    fn compiles_trivial_proto() {
        let staged = Stager::new()
            .add(
                "fixture/v1/x.proto",
                b"syntax = \"proto3\"; package fixture.v1; message Foo { string id = 1; }",
            )
            .stage()
            .unwrap();

        let out = compile_protos(&["fixture/v1/x.proto"], &[staged.path()]).expect("compile");

        assert!(!out.fds_bytes.is_empty());
        let foo = out
            .pool
            .get_message_by_name("fixture.v1.Foo")
            .expect("Foo in pool");
        assert_eq!(foo.fields().count(), 1);
    }
}
