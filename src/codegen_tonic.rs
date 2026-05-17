// SPDX-License-Identifier: MIT
//! Drive `tonic-prost-build` with `type_attribute(...)` injection.

use std::collections::BTreeMap;

use prost::Message as _;
use prost_types::FileDescriptorSet;

use crate::Error;

/// Run `tonic-prost-build` codegen on `fds_bytes`, applying every
/// `type_attribute(<fqn>, <attribute>)` from `type_attributes` before
/// invocation.
///
/// `builder_setup` lets callers configure the builder (server/client
/// flags, additional `type_attribute(...)`, `out_dir`, etc.) before
/// codegen runs. The closure receives a fresh
/// `tonic_prost_build::Builder` and returns it modified.
///
/// `type_attributes` is keyed by **un-dotted** fully-qualified name
/// (`my.v1.Foo`); this helper prepends the leading dot
/// (`.my.v1.Foo`) per `tonic-prost-build`'s convention.
///
/// # Errors
///
/// Returns [`Error::Tonic`] when `tonic-prost-build` codegen fails.
///
/// # Example
///
/// ```ignore
/// use proto_build_kit::{compile_protos, extract_method_string_extension,
///                       tonic_prost_build_with_attrs, Stager};
///
/// let staged = Stager::new().with(my_protos::files()).stage()?;
/// let out = compile_protos(&["my/v1/svc.proto"], &[staged.path()])?;
/// let etag_fields = extract_method_string_extension(&out.pool, "envelope.v1.etag_field");
///
/// let mut attrs: std::collections::BTreeMap<String, Vec<String>> = Default::default();
/// for (fqn, etag) in etag_fields {
///     attrs.entry(fqn).or_default().push(format!(
///         "#[derive(::service_kit::Envelope)] #[envelope(etag = \"{etag}\")]"
///     ));
/// }
///
/// tonic_prost_build_with_attrs(&out.fds_bytes, &attrs, |b| {
///     b.build_server(true).build_client(true)
/// })?;
/// ```
pub fn tonic_prost_build_with_attrs<F>(
    fds_bytes: &[u8],
    type_attributes: &BTreeMap<String, Vec<String>>,
    builder_setup: F,
) -> Result<(), Error>
where
    F: FnOnce(tonic_prost_build::Builder) -> tonic_prost_build::Builder,
{
    let mut builder = tonic_prost_build::configure();
    for (fqn, attrs) in type_attributes {
        let dotted = format!(".{fqn}");
        for attr in attrs {
            builder = builder.type_attribute(&dotted, attr);
        }
    }
    builder = builder_setup(builder);

    let fds = FileDescriptorSet::decode(fds_bytes)
        .map_err(|e| Error::Tonic(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?;

    builder
        .compile_fds(fds)
        .map_err(|e| Error::Tonic(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?;

    Ok(())
}
