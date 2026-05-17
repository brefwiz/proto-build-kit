// SPDX-License-Identifier: MIT
//! Read custom `MethodOptions` extension values from a `.proto` schema —
//! the mechanism every "envelope-style" annotation framework uses to
//! drive `#[derive(...)]` injection at build time.
//!
//! Demonstrates [`extract_method_string_extension`] alongside the
//! foundational primitives. This is the build-time half of the pattern;
//! the matching runtime would consume the resulting map to install
//! middleware that reads the annotated fields.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example extract_annotations
//! ```

use proto_build_kit::{Stager, compile_protos, extract_method_string_extension};

/// Declares a single custom `MethodOptions` extension named
/// `my.opts.etag_field`. Real-world annotation frameworks usually ship
/// this in a sibling crate that `*-protos`-style consumers depend on;
/// here it's inline for clarity.
const CONVENTIONS_PROTO: &[u8] = br#"
syntax = "proto3";
package my.opts;
import "google/protobuf/descriptor.proto";

extend google.protobuf.MethodOptions {
  // Marks which proto field on the RESPONSE carries an ETag value
  // suitable for optimistic concurrency / cache validation.
  optional string etag_field = 90001;
}
"#;

/// A user service whose `GetUser` RPC declares
/// `(my.opts.etag_field) = "version"` — the `User` response message's
/// `version` field is the `ETag` source.
const SERVICE_PROTO: &[u8] = br#"
syntax = "proto3";
package my.v1;
import "my/opts/conventions.proto";

service UserService {
  rpc GetUser(GetUserRequest) returns (User) {
    option (my.opts.etag_field) = "version";
  }
  rpc ListUsers(ListUsersRequest) returns (ListUsersResponse);
}

message GetUserRequest  { string id = 1; }
message User {
  string id      = 1;
  string name    = 2;
  uint64 version = 3;
}

message ListUsersRequest  { uint32 page_size = 1; }
message ListUsersResponse { repeated User items = 1; }
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Stage both protos: the annotation declaration AND the service
    //    that uses it. Both must be on the protoc include path; the
    //    service imports the annotation file by its package-relative
    //    path.
    let staged = Stager::new()
        .add("my/opts/conventions.proto", CONVENTIONS_PROTO)
        .add("my/v1/service.proto", SERVICE_PROTO)
        .stage()?;

    // 2. Compile both. The descriptor pool preserves the extension
    //    VALUES — encoding to FDS bytes would drop them, which is why
    //    we need the pool, not just the bytes.
    let out = compile_protos(
        &["my/v1/service.proto", "my/opts/conventions.proto"],
        &[staged.path()],
    )?;

    // 3. Walk the pool: for every method declaring the extension,
    //    record the response-message FQN ↔ extension value pair.
    let etag_fields = extract_method_string_extension(&out.pool, "my.opts.etag_field");

    println!("Methods declaring (my.opts.etag_field):");
    if etag_fields.is_empty() {
        println!("  (none)");
    } else {
        for (response_fqn, field_name) in &etag_fields {
            println!("  {response_fqn} → response field `{field_name}`");
        }
    }

    // 4. Real build scripts would pair this map with
    //    `tonic_prost_build_with_attrs` to inject derives onto the
    //    response types. See the README and the docs for
    //    `proto_build_kit::tonic_prost_build_with_attrs` for the
    //    full pipeline.
    Ok(())
}
