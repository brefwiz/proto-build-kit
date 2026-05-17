// SPDX-License-Identifier: MIT
//! Stage embedded proto bytes onto a tempdir at protoc-relative paths.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;

use crate::Error;

/// Accumulates `(relative_path, bytes)` pairs and writes them to a
/// fresh tempdir laid out as `<tmp>/<relative_path>`.
///
/// `relative_path` is the protoc-style import path the consuming
/// `.proto` will use (`bones/v1/pagination.proto`, etc.). The
/// returned [`tempfile::TempDir`] cleans up on drop — hold it until
/// codegen finishes.
///
/// # Examples
///
/// ```no_run
/// use proto_build_kit::Stager;
///
/// const FOO: &[u8] = b"syntax = \"proto3\"; package foo.v1; message Foo {}";
///
/// fn build() -> Result<(), Box<dyn std::error::Error>> {
///     let staged = Stager::new()
///         .add("foo/v1/foo.proto", FOO)
///         .stage()?;
///     // Use staged.path() on your protoc include path.
///     Ok(())
/// }
/// ```
///
/// Pair with a sibling `*-protos` crate that exposes `files()`:
///
/// ```ignore
/// let staged = Stager::new()
///     .with(some_protos::files())
///     .with(other_protos::files())
///     .stage()?;
/// ```
///
/// # Duplicate detection
///
/// Two entries with the same `relative_path` cause `stage()` to return
/// [`Error::DuplicatePath`]. This protects against silent shadowing when
/// two `*-protos` crates collide on a path — almost always a bug.
#[derive(Default)]
pub struct Stager {
    files: Vec<(&'static str, &'static [u8])>,
}

impl Stager {
    /// Construct an empty stager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a single `(relative_path, bytes)` entry.
    #[must_use]
    pub fn add(mut self, relative_path: &'static str, bytes: &'static [u8]) -> Self {
        self.files.push((relative_path, bytes));
        self
    }

    /// Append every entry yielded by `iter`. Typically called with a
    /// `*-protos` crate's `files()` accessor.
    #[must_use]
    pub fn with<I>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = (&'static str, &'static [u8])>,
    {
        self.files.extend(iter);
        self
    }

    /// Write every staged entry to a fresh tempdir and return the
    /// handle. Add `tempdir.path()` to your protoc include path.
    ///
    /// # Errors
    ///
    /// - [`Error::DuplicatePath`] if two entries declare the same
    ///   relative path.
    /// - [`Error::Io`] if tempdir creation, directory creation, or
    ///   file writes fail.
    pub fn stage(self) -> Result<tempfile::TempDir, Error> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (path, _) in &self.files {
            if !seen.insert(*path) {
                return Err(Error::DuplicatePath {
                    path: (*path).to_string(),
                });
            }
        }

        let dir = tempfile::tempdir()?;
        for (rel, bytes) in self.files {
            let target: PathBuf = dir.path().join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut f = std::fs::File::create(&target)?;
            f.write_all(bytes)?;
        }
        Ok(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_single_file_at_relative_path() {
        let dir = Stager::new()
            .add("a/v1/x.proto", b"syntax = \"proto3\";")
            .stage()
            .expect("stage");
        let p = dir.path().join("a/v1/x.proto");
        assert!(p.exists(), "expected staged file at {p:?}");
        let body = std::fs::read(&p).expect("read");
        assert_eq!(body.as_slice(), b"syntax = \"proto3\";");
    }

    #[test]
    fn stages_multiple_files_under_nested_subdirs() {
        let dir = Stager::new()
            .add("a/v1/x.proto", b"x")
            .add("b/v2/y.proto", b"y")
            .stage()
            .expect("stage");
        assert!(dir.path().join("a/v1/x.proto").exists());
        assert!(dir.path().join("b/v2/y.proto").exists());
    }

    #[test]
    fn with_iterator_extends_files() {
        let pairs: Vec<(&str, &[u8])> = vec![
            ("a/v1/x.proto", b"x" as &[u8]),
            ("b/v1/y.proto", b"y" as &[u8]),
        ];
        let dir = Stager::new().with(pairs).stage().expect("stage");
        assert!(dir.path().join("a/v1/x.proto").exists());
        assert!(dir.path().join("b/v1/y.proto").exists());
    }

    #[test]
    fn duplicate_paths_return_error() {
        let err = Stager::new()
            .add("a/v1/x.proto", b"first")
            .add("a/v1/x.proto", b"second")
            .stage()
            .expect_err("should error on duplicate");
        match err {
            Error::DuplicatePath { path } => assert_eq!(path, "a/v1/x.proto"),
            other => panic!("expected DuplicatePath, got {other:?}"),
        }
    }
}
