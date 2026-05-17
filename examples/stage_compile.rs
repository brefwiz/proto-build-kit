// SPDX-License-Identifier: MIT
//! Stage an embedded `.proto` blob onto a tempdir, compile it via `protox`,
//! and print the resulting descriptor structure.
//!
//! Demonstrates the two foundational primitives — [`Stager`] and
//! [`compile_protos`] — outside of a build-script context, useful for
//! inspecting what `proto-build-kit` actually does before wiring it into
//! your own `build.rs`.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example stage_compile
//! ```

use proto_build_kit::{Stager, compile_protos};

const HELLO_PROTO: &[u8] = br#"
syntax = "proto3";
package hello.v1;

service HelloService {
  rpc Greet(GreetRequest) returns (Greeting);
  rpc ListGreetings(ListGreetingsRequest) returns (ListGreetingsResponse);
}

message GreetRequest  { string name = 1; }
message Greeting      { string message = 1; }

message ListGreetingsRequest  { uint32 page_size = 1; }
message ListGreetingsResponse { repeated Greeting items = 1; }
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Stage the embedded proto onto a tempdir at its protoc-relative path.
    let staged = Stager::new()
        .add("hello/v1/hello.proto", HELLO_PROTO)
        .stage()?;
    println!("Staged proto at: {}", staged.path().display());

    // 2. Compile via protox; receive descriptor pool + FDS bytes.
    let out = compile_protos(&["hello/v1/hello.proto"], &[staged.path()])?;
    println!("Compiled. FDS size: {} bytes", out.fds_bytes.len());

    // 3. Walk the pool and print every service + method shape.
    for service in out.pool.services() {
        println!("\nService: {}", service.full_name());
        for method in service.methods() {
            println!(
                "  {} ({} → {})",
                method.name(),
                method.input().full_name(),
                method.output().full_name()
            );
        }
    }

    // 4. The tempdir auto-cleans on drop here.
    Ok(())
}
