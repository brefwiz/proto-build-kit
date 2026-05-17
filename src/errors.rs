// SPDX-License-Identifier: MIT

/// Errors surfaced by the `proto-build-kit` primitives.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// I/O failure (tempdir creation, file write, parent-dir mkdir).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// `Stager::stage()` detected two entries writing to the same
    /// protoc-relative path.
    #[error("duplicate proto staged at {path:?}")]
    DuplicatePath {
        /// The relative path declared twice.
        path: String,
    },

    /// `protox` failed to parse one of the `.proto` files.
    #[error("protox compile: {0}")]
    Protox(#[from] Box<protox::Error>),

    /// `prost-reflect` descriptor-pool construction failed.
    #[error("descriptor pool: {0}")]
    DescriptorPool(#[from] prost_reflect::DescriptorError),

    /// `tonic-prost-build` codegen failed (only surfaced when the
    /// `tonic` feature is enabled).
    #[cfg(feature = "tonic")]
    #[error("tonic-build codegen: {0}")]
    Tonic(#[source] Box<dyn std::error::Error + Send + Sync>),
}

// `protox::Error` is large; box it to keep the enum cheap to move.
impl From<protox::Error> for Error {
    fn from(e: protox::Error) -> Self {
        Error::Protox(Box::new(e))
    }
}
