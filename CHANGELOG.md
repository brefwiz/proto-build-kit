# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-05-17

### Added

- **`Stager`** builder — accumulates `(relative_path, &'static [u8])` pairs and writes them to a fresh tempdir at protoc-relative paths. Duplicate-path detection. Pair with sibling `*-protos` crates that expose proto bytes via `include_bytes!` and a `files()` accessor.
- **`compile_protos`** — `protox` wrapper returning the `prost-reflect` descriptor pool (preserves `MethodOptions` extension VALUES, which encoded `FileDescriptorSet` bytes drop) plus FDS bytes ready for downstream codegen drivers.
- **`extract_method_string_extension`** — walks every method declared in a descriptor pool, reads a string-typed `MethodOptions` extension by FQN, returns a map keyed by response-message FQN. Empty when no methods declare the extension; first-encountered wins on shared response types.
- **`tonic_prost_build_with_attrs`** (feature: `tonic`, default-on) — drives `tonic-prost-build` with `type_attribute(...)` injection from an annotation→attribute map.

### Features

- `tonic` (default-on) — enables the `tonic-prost-build` wrapper. Disable to drive your own codegen (`connectrpc-build`, custom) and only use stage / compile / extract.

### Tests

- 8 unit tests across the four modules.
- 1 end-to-end test (`end_to_end_envelope_annotation_pipeline`) that drives a fake `.proto` schema through every primitive and asserts the generated Rust contains the injected `#[fake_envelope(...)]` attribute on the expected struct and no others.
