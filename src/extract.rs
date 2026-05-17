// SPDX-License-Identifier: MIT
//! Read `MethodOptions` extension values from a descriptor pool.
//!
//! Custom proto `extend google.protobuf.MethodOptions` extensions store
//! their VALUES in the binary `MethodOptions` payload. Encoding the
//! whole `FileDescriptorSet` to bytes (the standard codegen-input form)
//! drops them — the bytes survive, but they're treated as unknown
//! fields on the consumer side. To read them at build time, walk the
//! descriptor POOL instead (which `protox` builds with `prost-reflect`,
//! preserving the extension VALUES).
//!
//! This module provides the generic walker: "for every method in the
//! pool, look up extension `<fqn>` and, when present, record its
//! string value indexed by the method's response-message FQN."

use std::collections::BTreeMap;

use prost_reflect::Value;

/// Walk every method declared in `pool`, look for the
/// `MethodOptions`-level extension named `extension_fqn`, and return a
/// map keyed by **response-message FQN** (e.g. `my.v1.Resource`) with
/// the extension's string value.
///
/// Methods that don't declare the extension are skipped silently.
/// Multiple methods returning the same response type with different
/// extension values: **first encountered wins** (per pool iteration
/// order). Returning the same value from every method is the convention
/// — mismatch is a service-author bug worth catching with a
/// conformance test.
///
/// Returns an empty map when no methods declare the extension.
///
/// # Example
///
/// ```ignore
/// // Given a .proto with:
/// //   import "envelope/v1/conventions.proto";
/// //   service UserService {
/// //     rpc GetUser(GetUserRequest) returns (User) {
/// //       option (envelope.v1.etag_field) = "version";
/// //     }
/// //   }
/// let out = compile_protos(&["user.proto"], &[staged.path()])?;
/// let etag_fields = extract_method_string_extension(&out.pool, "envelope.v1.etag_field");
/// assert_eq!(etag_fields.get("my.v1.User"), Some(&"version".to_string()));
/// ```
#[must_use]
pub fn extract_method_string_extension(
    pool: &prost_reflect::DescriptorPool,
    extension_fqn: &str,
) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();

    for service in pool.services() {
        for method in service.methods() {
            let response_fqn = method.output().full_name().to_string();
            if let Some(value) = read_string_extension(&method.options(), extension_fqn) {
                out.entry(response_fqn).or_insert(value);
            }
        }
    }

    out
}

fn read_string_extension(opts: &prost_reflect::DynamicMessage, fqn: &str) -> Option<String> {
    use prost_reflect::ReflectMessage as _;
    let pool = opts.descriptor().parent_pool().clone();
    let ext = pool.get_extension_by_name(fqn)?;
    if !opts.has_extension(&ext) {
        return None;
    }
    let value = opts.get_extension(&ext);
    match &*value {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Stager, compile_protos};

    const CONVENTIONS_PROTO: &[u8] = br#"
syntax = "proto3";
package fixture.opts;
import "google/protobuf/descriptor.proto";
extend google.protobuf.MethodOptions {
  optional string etag_field = 60001;
  optional string location_field = 60002;
}
"#;

    fn stage_conv() -> tempfile::TempDir {
        Stager::new()
            .add("fixture/opts/conventions.proto", CONVENTIONS_PROTO)
            .stage()
            .unwrap()
    }

    #[test]
    fn extracts_string_extension_value() {
        let proto = br#"
syntax = "proto3";
package fixture.v1;
import "fixture/opts/conventions.proto";

service Svc {
  rpc Get(GetReq) returns (User) {
    option (fixture.opts.etag_field) = "version";
  }
}
message GetReq { string id = 1; }
message User   { string id = 1; uint64 version = 2; }
"#;
        let staged_proto = Stager::new()
            .add("fixture/v1/x.proto", proto)
            .stage()
            .unwrap();
        let staged_conv = stage_conv();

        let out = compile_protos(
            &["fixture/v1/x.proto", "fixture/opts/conventions.proto"],
            &[staged_proto.path(), staged_conv.path()],
        )
        .expect("compile");

        let map = extract_method_string_extension(&out.pool, "fixture.opts.etag_field");
        assert_eq!(map.get("fixture.v1.User"), Some(&"version".to_string()));
    }

    #[test]
    fn returns_empty_map_when_no_methods_declare_extension() {
        let proto = br#"
syntax = "proto3";
package fixture.v1;

service Svc {
  rpc Get(GetReq) returns (Resp);
}
message GetReq { string id = 1; }
message Resp   { string body = 1; }
"#;
        let staged = Stager::new()
            .add("fixture/v1/x.proto", proto)
            .stage()
            .unwrap();
        let out = compile_protos(&["fixture/v1/x.proto"], &[staged.path()]).expect("compile");
        let map = extract_method_string_extension(&out.pool, "fixture.opts.etag_field");
        assert!(map.is_empty());
    }

    #[test]
    fn first_method_wins_when_multiple_share_response_type() {
        let proto = br#"
syntax = "proto3";
package fixture.v1;
import "fixture/opts/conventions.proto";

service Svc {
  rpc First(Req)  returns (Shared) { option (fixture.opts.etag_field) = "v1"; }
  rpc Second(Req) returns (Shared) { option (fixture.opts.etag_field) = "v2"; }
}
message Req    { string id = 1; }
message Shared { string id = 1; }
"#;
        let staged_proto = Stager::new()
            .add("fixture/v1/x.proto", proto)
            .stage()
            .unwrap();
        let staged_conv = stage_conv();
        let out = compile_protos(
            &["fixture/v1/x.proto", "fixture/opts/conventions.proto"],
            &[staged_proto.path(), staged_conv.path()],
        )
        .expect("compile");
        let map = extract_method_string_extension(&out.pool, "fixture.opts.etag_field");
        assert_eq!(map.get("fixture.v1.Shared"), Some(&"v1".to_string()));
    }
}
