//! The math for the hyperbezier curve family.

use kurbo::common as coeffs;
use kurbo::{Affine, BezPath, CubicBez, ParamCurveArclen, Point, Vec2};

use crate::util;

/// Parameters for a hyperbezier curve.
///
/// A hyperbezier is a curve defined by curvature as a function of arclength.
/// It is similar to the Spiro curve in this way, but for some of the parameter
/// space the function is different.
///
/// The parameter space is four dimensional. It is broken down symmetrically
/// into two parameters that predominantly affect one side of the curve, and
/// the curvature contributions are added:
///
/// k(s) = k0 * f(bias0, 1 - s) + k1 * f(bias1, s)
///
/// The "f" function takes a bias parameter, which can also be thought of as
/// tension. This value ranges from about -1 to exactly 2, with 2 representing
/// a cusp (infinitely high tension at the endpoint). For bias values less than
/// 1, it is defined thus:
///
/// f(bias, s) = s + 6 * (1 - bias) * (s^2 - s^3 - s)
///
/// For bias values greater than one, it is defined thus:
///
/// f(bias, s) = c * s / (1 + (bias - 1) * s) ^ 2
///
/// Here, c is a normalization term chosen so that the integral of f from s=0
/// to s=1 is 1.
///
/// A few observation. If both bias values are 1, then the curve is an Euler
/// spiral. If both bias values are less than 1, then curvature is a cubic
/// polynomial as a function of arclength, so it is a Spiro curve.
pub struct HyperBezier {
    pub k0: f64,
    pub bias0: f64,
    pub k1: f64,
    pub bias1: f64,
}

/// An intermediate parametrization of the curve family.
///
/// Here, angles are given relative to the chord, but the bias parameters
/// are the same as for `HyperBezier`.
pub struct ThetaParams {
    pub th0: f64,
    pub bias0: f64,
    pub th1: f64,
    pub bias1: f64,
}

/// Result of measuring the curve.
///
/// The `th0` and `th1` values are defined so that if they are have the
/// same sign, the curve is convex, but if they are opposite signs, it is
/// an "s" shape.
#[derive(Copy, Clone)]
pub struct HyperBezierResult {
    /// Tangent angle from the chord to the curve at the start point.
    pub th0: f64,
    /// Tangent angle from the chord to the curve at the end point.
    pub th1: f64,
    /// Length of the chord assuming total arclength = 1.
    pub chord: f64,
    pub k0: f64,
    pub k1: f64,
}

impl HyperBezier {
    /// Compute the angle for the given parameter.
    ///
    /// The argument is an arclength parametrization, ranging from 0 to 1.
    ///
    /// The returned angle is relative only, in other words there could be an
    /// arbitrary rotation of the entire curve.
    pub fn compute_theta(&self, s: f64) -> f64 {
        self.k1 * integrate_basis(self.bias1, s) - self.k0 * integrate_basis(self.bias0, 1.0 - s)
    }

    /// Compute the endpoint tangent angles and the chord length.
    pub fn compute(&self) -> HyperBezierResult {
        let integral = self.integrate(0.0, 1.0, 24);
        let th_chord = integral.atan2();
        let chord = integral.hypot();
        let th0 = th_chord - self.compute_theta(0.0);
        let th1 = self.compute_theta(1.0) - th_chord;
        let k0 = chord * self.k0 * compute_k(self.bias0);
        let k1 = chord * self.k1 * compute_k(self.bias1);
        HyperBezierResult {
            th0,
            th1,
            chord,
            k0,
            k1,
        }
    }

    fn integrate(&self, t0: f64, t1: f64, order: usize) -> Vec2 {
        let c = match order {
            3 => coeffs::GAUSS_LEGENDRE_COEFFS_3,
            5 => coeffs::GAUSS_LEGENDRE_COEFFS_5,
            7 => coeffs::GAUSS_LEGENDRE_COEFFS_7,
            9 => coeffs::GAUSS_LEGENDRE_COEFFS_9,
            11 => coeffs::GAUSS_LEGENDRE_COEFFS_11,
            24 => coeffs::GAUSS_LEGENDRE_COEFFS_24,
            _ => panic!("don't have coefficients for {}", order),
        };
        let mut result = Vec2::ZERO;
        let tm = 0.5 * (t1 + t0);
        let dt = 0.5 * (t1 - t0);
        for (wi, xi) in c {
            let t = tm + dt * xi;
            let th = self.compute_theta(t);
            result += *wi * Vec2::from_angle(th);
        }
        dt * result
    }

    /// Render to beziers.
    ///
    /// The current algorithm just does a fixed subdivision based on arclength,
    /// but should be adaptive in several ways; more subdivision for twistier
    /// curves, and also more sophisticated parametrization (important as tension
    /// increases).
    pub fn render(&self, n: usize) -> BezPath {
        let order = 24;
        let v = self.integrate(0.0, 1.0, order);
        let a = Affine::new([v.x, v.y, -v.y, v.x, 0.0, 0.0]).inverse();
        let step = 1.0 / (n as f64);
        let mut result = BezPath::new();
        let mut last_p = Point::ZERO;
        let mut last_v = step * (1.0 / 3.0) * Vec2::from_angle(self.compute_theta(0.0));
        result.move_to(last_p);
        for i in 1..=n {
            let t = (i as f64) * step;
            let p = self.integrate(0.0, t, order).to_point();
            let p1 = last_p + last_v;
            let v = step * (1.0 / 3.0) * Vec2::from_angle(self.compute_theta(t));
            let p2 = p - v;
            result.curve_to(a * p1, a * p2, a * p);
            last_v = v;
            last_p = p;
        }
        result
    }

    /// Solve for curve params, given theta params.
    pub fn solve_for_theta(params: &ThetaParams) -> HyperBezier {
        let ThetaParams {
            th0,
            bias0,
            th1,
            bias1,
        } = *params;
        let mut dth = 0.0;
        let mut lastxy: Option<(f64, f64)> = None;
        const N: usize = 10;
        for i in 0..N {
            let params = HyperBezier {
                k0: th0 + 0.5 * dth,
                bias0,
                k1: th1 - 0.5 * dth,
                bias1,
            };
            if i == N - 1 {
                return params;
            }
            let result = params.compute();
            let th_err = util::mod_tau(th0 - th1 - (result.th0 - result.th1));
            if th_err.abs() < 1e-3 {
                return params;
            }
            // Secant method
            let nextxy = (dth, th_err);
            let delta = if let Some(lastxy) = lastxy {
                (nextxy.0 - lastxy.0) / (nextxy.1 - lastxy.1)
            } else {
                -0.5
            };
            dth -= delta * th_err;
            lastxy = Some(nextxy);
        }
        unreachable!()
    }

    /// Solve for curve params, given bezier control points.
    ///
    /// The points are given relative to p0 at (0, 0) and p3 at
    /// (1, 0).
    pub fn solve(p1: Point, p2: Point) -> HyperBezier {
        fn inv_arm_len(h: f64, chord: f64) -> f64 {
            let a = h * 3.0 * chord.powf(1.0);
            if a < 1.0 {
                2.0 - a.powf(2.0)
            } else {
                1.0 + 2.0 * (0.5 * (1.0 - a)).tanh()
            }
        }
        let v1 = p1.to_vec2();
        let v2 = Point::new(1.0, 0.0) - p2;
        let c = CubicBez::new(Point::ORIGIN, p1, p2, Point::new(1.0, 0.0));
        // TODO: signs feel reversed here, but it all works out in the end.
        let th0 = -v1.atan2();
        let th1 = v2.atan2();
        let chord = 1.0 / c.arclen(1e-3);
        let bias0 = inv_arm_len(v1.hypot(), chord);
        let bias1 = inv_arm_len(v2.hypot(), chord);
        let theta_params = ThetaParams {
            th0,
            bias0,
            th1,
            bias1,
        };
        Self::solve_for_theta(&theta_params)
    }
}

const MAX_A: f64 = 1.0 - 1e-4;

/// Compute integral of basis function.
///
/// The integral of the basis function can be represented as a reasonably
/// simple closed-form analytic formula.
///
/// Note: this is normalized so that f(1) - f(0) = 1.
///
/// This is oriented for the rightmost control point.
fn integrate_basis(bias: f64, s: f64) -> f64 {
    if bias > 1.0 {
        let a = (bias - 1.0).min(MAX_A);
        let norm = 1.0 / (1.0 - a) + (1.0 - a).ln() - 1.0;
        (1.0 / (1.0 - a * s) + (1.0 - a * s).ln()) / norm
    } else {
        let iy0 = 4.0 * s.powi(3) - 3.0 * s.powi(4);
        let iy1 = s.powi(2);
        iy0 + bias * (iy1 - iy0)
    }
}

/// Compute curvature at endpoint.
fn compute_k(bias: f64) -> f64 {
    if bias <= 1.0 {
        bias * 2.0
    } else {
        let a = (bias - 1.0).min(MAX_A);
        // Reciprocal of integral
        let sr = (a * a) / (1.0 / (1.0 - a) + (1.0 - a).ln() - 1.0);
        sr / (1.0 - a).powi(2)
    }
}
