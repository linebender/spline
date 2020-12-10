//! Generate an SVG from a json description of a spline.
//!
//! This is intended to be used as a debugging tool. To generate the input
//! data, use serde_json to a list of `SplineSpec` objects.

use kurbo::{BezPath, Point};

use spline::{Element, SplineSpec};

fn main() {
    let path = std::env::args().skip(1).next().expect("needs filename");
    let data = std::fs::read_to_string(path).unwrap();
    let mut splines: Vec<SplineSpec> = serde_json::from_str(&data).unwrap();
    let mut bp = BezPath::new();
    for spec in &mut splines {
        let spline = spec.solve();
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
    for spec in &mut splines {
        let spline = spec.solve();
        // first draw the points in the segments, which will include auto points
        for seg in spline.segments() {
            print_point_stroke(seg.p1, "grey");
            print_point_stroke(seg.p2, "grey");
            print_line(seg.p0, seg.p1);
            print_line(seg.p2, seg.p3);
        }
        // then draw the points from the elements, which will only be non-auto
        // we keep track of points to draw handles
        for el in spec.elements() {
            match el {
                Element::MoveTo(pt) => print_point_fill(*pt, "blue"),
                Element::LineTo(pt, true) => print_point_fill(*pt, "green"),
                Element::LineTo(pt, false) => print_point_fill(*pt, "blue"),
                Element::SplineTo(p1, p2, p3, smooth) => {
                    if let Some(p1) = p1 {
                        print_point_fill(*p1, "grey");
                    }
                    if let Some(p2) = p2 {
                        print_point_fill(*p2, "grey");
                    }
                    let color = if *smooth { "green" } else { "blue" };
                    print_point_fill(*p3, color);
                }
            }
        }
    }

    println!(
        r#"    </svg>
    </body>
</html>"#
    );
}

fn print_line(p1: Point, p2: Point) {
    println!(
        r#"      <line x1="{}" y1="{}", x2="{}" y2="{}" style="stroke:grey;stroke-wdith:1"  />"#,
        p1.x, p1.y, p2.x, p2.y
    );
}
fn print_point_fill(point: Point, color: &str) {
    println!(
        r#"      <circle cx="{}" cy="{}" r="3" fill="{}" />"#,
        point.x, point.y, color
    );
}

fn print_point_stroke(point: Point, color: &str) {
    println!(
        r#"      <circle cx="{}" cy="{}" r="3" fill="white" stroke="{}" stroke-width="1" />"#,
        point.x, point.y, color
    );
}
