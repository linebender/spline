//! A spline for interactive curve design.
//!
//! This crate is intended to supply an interpolating spline with similar
//! scope as the "[research spline]" and [Spiro]. It is built on top of the
//! "hyperbezier" curve.
//!
//! At the moment, this crate only contains the underlying curve. Hopefully,
//! the interpolating spline logic will follow in good time.
//!
//! [Spiro]: https://github.com/raphlinus/spiro
//! [research spline]: https://github.com/raphlinus/spline-research

mod hyperbezier;
mod simple_spline;
mod util;

pub use hyperbezier::{HyperBezier, ThetaParams};
pub use simple_spline::SimpleSpline;
