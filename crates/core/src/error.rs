pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Internal array error: {0}")]
    InternalArrayError(#[from] ndarray::ShapeError),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Image buffer error: {0}")]
    ImageBuffer(#[from] fast_image_resize::ImageBufferError),
    #[error("Different types of pixels: {0}")]
    DifferentTypesOfPixels(#[from] fast_image_resize::DifferentTypesOfPixelsError),
    #[error("Tiff error: {0}")]
    Tiff(#[from] tiff::TiffError),
}
