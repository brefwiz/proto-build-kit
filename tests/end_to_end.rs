// SPDX-License-Identifier: MIT
//! End-to-end test: fake `.proto` schema → [`Stager`] → [`compile_protos`] →
//! [`extract_method_string_extension`] → [`tonic_prost_build_with_attrs`] →
//! assert the generated Rust contains the injected attribute on the
//! right struct.
//!
//! This exercises every primitive in one pass — the contract the
//! library promises consumers.

#![cfg(feature = "tonic")]

use std::collections::BTreeMap;

use proto_build_kit::{
    Stager, compile_protos, extract_method_string_extension, tonic_prost_build_with_attrs,
};

/// Fake annotation file. Mirrors the shape real envelope-style
/// annotation protos take — a single `MethodOptions` extension.
const CONVENTIONS_PROTO: &[u8] = br#"
syntax = "proto3";
package fake.opts;
import "google/protobuf/descriptor.proto";

extend google.protobuf.MethodOptions {
  // Marks which proto field on the RESPONSE carries the ETag value.
  optional string etag_field = 70001;
  // Marks which proto field carries the Location-like value.
  optional string location_field = 70002;
}
"#;

/// Fake service `.proto` declaring two RPCs:
/// - `GetItem` annotated with `etag_field = "version"` on response `Item`.
/// - `CreateItem` annotated with `location_field = "id"` on response `Item`.
///
/// Both return `Item`; the extractor should merge: `Item` ↔
/// etag=version + location=id.
const SERVICE_PROTO: &[u8] = br#"
syntax = "proto3";
package fake.v1;
import "fake/opts/conventions.proto";

service FakeService {
  rpc GetItem(GetItemRequest) returns (Item) {
    option (fake.opts.etag_field) = "version";
  }
  rpc CreateItem(CreateItemRequest) returns (Item) {
    option (fake.opts.location_field) = "id";
  }
}

message GetItemRequest    { string id = 1; }
message CreateItemRequest { string name = 1; }
message Item {
  string id      = 1;
  uint64 version = 2;
  string name    = 3;
}
"#;

#[test]
fn end_to_end_envelope_annotation_pipeline() {
    // 1. Stage the fake schema onto a tempdir at protoc-relative paths.
    let staged = Stager::new()
        .add("fake/opts/conventions.proto", CONVENTIONS_PROTO)
        .add("fake/v1/service.proto", SERVICE_PROTO)
        .stage()
        .expect("stage");

    // 2. Compile via protox; pool preserves the extension values.
    let compiled = compile_protos(
        &["fake/v1/service.proto", "fake/opts/conventions.proto"],
        &[staged.path()],
    )
    .expect("compile_protos");
    assert!(
        !compiled.fds_bytes.is_empty(),
        "expected non-empty FDS bytes"
    );

    // 3. Extract annotation values, one extension at a time.
    let etag = extract_method_string_extension(&compiled.pool, "fake.opts.etag_field");
    let location = extract_method_string_extension(&compiled.pool, "fake.opts.location_field");

    assert_eq!(
        etag.get("fake.v1.Item"),
        Some(&"version".to_string()),
        "GetItem's etag_field=version should map to Item; etag map = {etag:?}"
    );
    assert_eq!(
        location.get("fake.v1.Item"),
        Some(&"id".to_string()),
        "CreateItem's location_field=id should map to Item; location map = {location:?}"
    );

    // 4. Merge into a per-response attribute map. One attribute per
    //    response type that union-combines both annotations.
    let mut attrs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let responses: std::collections::BTreeSet<&String> =
        etag.keys().chain(location.keys()).collect();
    for response_fqn in responses {
        let mut parts = Vec::new();
        if let Some(v) = etag.get(response_fqn) {
            parts.push(format!("etag = \"{v}\""));
        }
        if let Some(v) = location.get(response_fqn) {
            parts.push(format!("location = \"{v}\""));
        }
        attrs
            .entry(response_fqn.clone())
            .or_default()
            .push(format!("#[fake_envelope({})]", parts.join(", ")));
    }

    // 5. Run tonic-prost-build into a controlled out-dir.
    let codegen_out = tempfile::tempdir().expect("codegen tempdir");
    tonic_prost_build_with_attrs(&compiled.fds_bytes, &attrs, |b| {
        b.out_dir(codegen_out.path())
            .build_server(true)
            .build_client(true)
    })
    .expect("tonic_prost_build_with_attrs");

    // 6. Read the generated file and assert the attribute landed on Item.
    let generated = std::fs::read_to_string(codegen_out.path().join("fake.v1.rs"))
        .expect("read generated fake.v1.rs");

    assert!(
        generated.contains("pub struct Item"),
        "expected `pub struct Item` in generated code; got:\n{generated}"
    );
    assert!(
        generated.contains("#[fake_envelope(etag = \"version\", location = \"id\")]"),
        "expected injected attribute on Item; got:\n{generated}"
    );

    // 7. Negative: request types (`GetItemRequest`, `CreateItemRequest`)
    //    are not response types for any annotated method, so they must
    //    not receive the attribute. Exactly ONE injection expected (on
    //    `Item`).
    let injection_count = generated.matches("#[fake_envelope(").count();
    assert_eq!(
        injection_count, 1,
        "expected exactly one #[fake_envelope(...)] injection (on Item); \
         got {injection_count} in:\n{generated}"
    );
}
