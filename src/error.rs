/// Errors that can occur during OpenCASCADE operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// STEP file read failed (invalid format or corrupted data).
    #[error("STEP read failed")]
    StepReadFailed,

    /// BRep file read failed (invalid format or corrupted data).
    #[error("BRep read failed")]
    BrepReadFailed,

    /// STEP file write failed.
    #[error("STEP write failed")]
    StepWriteFailed,

    /// BRep file write failed.
    #[error("BRep write failed")]
    BrepWriteFailed,

    /// Triangulation/meshing failed.
    #[error("Triangulation failed")]
    TriangulationFailed,
}
