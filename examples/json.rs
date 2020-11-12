use serde::Deserialize;

use kurbo::BezPath;

use spline::{Spline, SplineSpec};

#[derive(Deserialize, Debug)]
struct Path {
    subpaths: Vec<Subpath>,
}

#[derive(Deserialize, Debug)]
struct Subpath {
    pts: Vec<Point>,
    #[serde(default)]
    is_closed: bool,
}

#[derive(Clone, Copy, Deserialize, Debug)]
enum Point {
    OnCurve(f64, f64, bool),
    Auto,
    OffCurve(f64, f64),
}

impl Point {
    fn to_kurbo(&self) -> Option<kurbo::Point> {
        match self {
            Point::OnCurve(x, y, _) => Some(kurbo::Point::new(*x, *y)),
            Point::OffCurve(x, y) => Some(kurbo::Point::new(*x, *y)),
            _ => None,
        }
    }

    fn is_smooth(&self) -> bool {
        match self {
            Point::OnCurve(_, _, is_smooth) => *is_smooth,
            _ => false,
        }
    }
}

fn subpath_to_spline(p: &Subpath) -> Spline {
    let mut spec = SplineSpec::new();
    spec.move_to(p.pts[0].to_kurbo().unwrap());
    let mut i = 1;
    while i < p.pts.len() {
        if matches!(p.pts[i], Point::OffCurve(..) | Point::Auto) {
            let last_pt = p.pts[(i + 2) % p.pts.len()];
            spec.spline_to(p.pts[i].to_kurbo(), p.pts[i + 1].to_kurbo(), last_pt.to_kurbo().unwrap(), last_pt.is_smooth());
            i += 3;
        } else if let Point::OnCurve(x, y, is_smooth) = p.pts[i] {
            spec.line_to(kurbo::Point::new(x, y), is_smooth);
            i += 1;
        }
    }
    if p.is_closed {
        spec.close();
    }
    spec.solve()
}

fn path_to_splines(p: &Path) -> Vec<Spline> {
    p.subpaths.iter().map(subpath_to_spline).collect()
}

fn main() {
    let path = std::env::args().skip(1).next().expect("needs filename");
    let data = std::fs::read_to_string(path).unwrap();
    let path: Path = serde_json::from_str(&data).unwrap();
    let splines = path_to_splines(&path);
    let mut bp = BezPath::new();
    for spline in &splines {
        spline.render_extend(&mut bp);
    }
    println!(
        r##"<!DOCTYPE html>
<html>
    <body>
    <svg height="500" width="500">
      <path d="{}" fill="none" stroke="#000" />"
    "##,
        bp.to_svg()
    );
    println!(
        r#"    </svg>
    </body>
</html>"#
    );
}
