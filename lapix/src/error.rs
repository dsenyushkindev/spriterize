use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to generate image from raw data")]
    FailedImageFromRaw,
    #[error("No free image found")]
    MissingFreeImage,
    #[error("Unsupported image format")]
    UnsupportedImageFormat,
    #[error("Drawing action has not started")]
    DrawingNotStarted,
    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Bug: reversal list is not set")]
    ReversalNotSet,
    #[error("Codec error: {0}")]
    CodecError(#[from] bincode::Error),
    #[error("Invalid palette file: {0}")]
    InvalidPalette(String),
    #[error("A {cols}x{rows} sheet holds {} layers, but there are {layers}", cols * rows)]
    SheetTooSmall { cols: u32, rows: u32, layers: u32 },
}
