//! A simple interpolating spline.

use std::f64::consts::PI;

use kurbo::{Affine, BezPath, Point, Vec2};

use crate::hyperbezier::{HyperBezier, HyperBezierResult, ThetaParams};
use crate::util;

pub struct SimpleSpline {
    pts: Vec<Point>,
    ths: Vec<f64>,
}

struct Seg {
    th0: f64,
    th1: f64,
    d: Vec2,
}

impl SimpleSpline {
    pub fn new(pts: Vec<Point>) -> SimpleSpline {
        let ths = Self::initial_ths(&pts);
        SimpleSpline { pts, ths }
    }

    fn initial_ths(pts: &[Point]) -> Vec<f64> {
        let n = pts.len();
        let mut ths = vec![0.0; n];
        for i in 1..n - 1 {
            let d0 = pts[i] - pts[i - 1];
            let d1 = pts[i + 1] - pts[i];
            let th0 = d0.atan2();
            let th1 = d1.atan2();
            let bend = util::mod_tau(th1 - th0);
            // This is a bit different than the research spline, but is
            // intended to ensure that the chord angle never exceeds pi/2.
            let th = util::mod_tau(th0 + 0.5 * bend);
            ths[i] = th;
            if i == 1 {
                ths[0] = th0;
            }
            if i == n - 2 {
                ths[i + 1] = th1;
            }
        }
        ths
    }

    /// Render the spline to a bezier path.
    ///
    /// This should probably take an accuracy parameter as well, but
    /// at the moment there are accuracy problems, particularly at
    /// high tension.
    pub fn render(&self) -> BezPath {
        let mut result = BezPath::new();
        result.move_to(self.pts[0]);
        for i in 0..self.pts.len() - 1 {
            let (hb, seg) = self.get_hyperbezier(i);
            let p = self.pts[i];
            let d = seg.d;
            let a = Affine::new([d.x, d.y, -d.y, d.x, p.x, p.y]);
            let curve = hb.render(64);
            for el in curve.elements().iter().skip(1) {
                result.push(a * *el);
            }
        }
        result
    }

    fn get_seg(&self, i: usize) -> Seg {
        let d = self.pts[i + 1] - self.pts[i];
        let th = d.atan2();
        let th0 = util::mod_tau(self.ths[i] - th);
        let th1 = util::mod_tau(th - self.ths[i + 1]);
        Seg { th0, th1, d }
    }

    fn get_hyperbezier(&self, i: usize) -> (HyperBezier, Seg) {
        let seg = self.get_seg(i);
        // TODO: probably invert these signs in solve_for_theta
        let params = ThetaParams {
            th0: -seg.th0,
            bias0: bias_for_theta(seg.th0),
            th1: -seg.th1,
            bias1: bias_for_theta(seg.th1),
        };
        (HyperBezier::solve_for_theta(&params), seg)
    }

    fn compute_curvature(th0: f64, th1: f64) -> HyperBezierResult {
        let params = ThetaParams {
            th0: -th0,
            bias0: bias_for_theta(th0),
            th1: -th1,
            bias1: bias_for_theta(th1),
        };
        let hb = HyperBezier::solve_for_theta(&params);
        hb.compute()
    }

    /// Perform one iteration step.
    ///
    /// The current implementation is somewhat janky; it's mostly based
    /// on the research spline, which is kinda good enough.
    ///
    /// Returns the absolute error.
    pub fn iterate(&mut self, iter_ix: usize) -> f64 {
        let n = self.pts.len();
        if n < 3 {
            return 0.0;
        }
        // Fix endpoint tangents
        let seg0 = self.get_seg(0);
        self.ths[0] += endpoint_tangent(seg0.th1) - seg0.th0;
        let seg0 = self.get_seg(n - 2);
        self.ths[n - 1] -= endpoint_tangent(seg0.th0) - seg0.th1;

        let mut abs_err = 0.0;
        let mut x = vec![0.0; n - 2];
        let (hb, mut seg0) = self.get_hyperbezier(0);
        let mut r0 = hb.compute();
        let mut ch0 = seg0.d.hypot();
        for i in 0..n - 2 {
            let (hb, seg1) = self.get_hyperbezier(i + 1);
            let r1 = hb.compute();
            let ch1 = seg1.d.hypot();
            let err = compute_err(ch0, r0, ch1, r1);
            abs_err += err.abs();

            const EPSILON: f64 = 1e-3;
            let ak0p = Self::compute_curvature(seg0.th0, seg0.th1 + EPSILON);
            let ak1p = Self::compute_curvature(seg1.th0 - EPSILON, seg1.th1);
            let errp = compute_err(ch0, ak0p, ch1, ak1p);
            let derr = (errp - err) * (1.0 / EPSILON);

            x[i] = err / derr;

            r0 = r1;
            ch0 = ch1;
            seg0 = seg1;
        }
        let scale = (0.25 * (iter_ix as f64 + 1.0)).tanh();
        for i in 0..n - 2 {
            self.ths[i + 1] += scale * x[i];
        }
        abs_err
    }
}

pub(crate) fn bias_for_theta(th: f64) -> f64 {
    // Tangent angles up to this limit will be Euler spirals.
    const EULER_LIMIT: f64 = 0.3 * PI;
    let th = th.abs();
    if th < EULER_LIMIT {
        1.0
    } else {
        let len = 1.0 - (th - EULER_LIMIT) / (0.5 * PI - EULER_LIMIT);
        2.0 - len.powi(2)
    }
}

/// The tangent of an endpoint given the other tangent.
pub(crate) fn endpoint_tangent(th: f64) -> f64 {
    0.5 * (2.0 * th).sin()
}

fn compute_err(ch0: f64, ak0: HyperBezierResult, ch1: f64, ak1: HyperBezierResult) -> f64 {
    let ak0k1 = ak0.k1.atan();
    let ak1k0 = ak1.k0.atan();
    // rescale tangents by geometric mean of chordlengths
    let ch0 = ch0.sqrt();
    let ch1 = ch1.sqrt();
    let a0 = (ak0k1.sin() * ch1).atan2(ak0k1.cos() * ch0);
    let a1 = (ak1k0.sin() * ch0).atan2(ak1k0.cos() * ch1);
    a0 - a1
}
