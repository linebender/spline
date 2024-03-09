use kurbo::{fit_to_bezpath, ParamCurve, Point};
use spline::hyperbezier::{HyperbezParams, Hyperbezier};

fn main() {
    let params = HyperbezParams::new(-10., 5., 2.0, -2.0);
    let p0 = Point::new(100., 100.);
    let p1 = Point::new(300., 200.);
    let hb = Hyperbezier::from_points_params(params, p0, p1);
    println!("{hb:?}");
    let p = fit_to_bezpath(&hb, 1.0);
    println!("{}", p.to_svg());
    let p = fit_to_bezpath(&hb.subsegment(0.1 .. 0.8), 1.0);
    println!("{}", p.to_svg());
}