//! Thrust core functionalities.
//!

pub mod data;
pub mod intervals;
pub mod error;

#[cfg(feature = "polars")]
pub mod kalman;

pub use error::ThrustError;
