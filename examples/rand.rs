//! A simple test program that creates a random spline.
//!
//! This creates an interpolating spline from a sequence of random points,
//! and outputs an SVG.

use rand::distributions::{Distribution, Uniform};

use kurbo::Point;

use spline::Spline;

fn main() {
    let mut rng = rand::thread_rng();
    const N: usize = 10;
    let pts = (0..N)
        .map(|_| {
            let x = Uniform::from(0.0..500.0).sample(&mut rng);
            let y = Uniform::from(0.0..500.0).sample(&mut rng);
            Point::new(x, y)
        })
        .collect::<Vec<_>>();
    let mut spline = Spline::new(pts.clone());
    for iter_ix in 0..10 {
        let abs_err = spline.iterate(iter_ix);
        eprintln!("err: {}", abs_err);
    }
    let path = spline.render();
    println!(
        r##"<!DOCTYPE html>
    <html>
    <body>
    <svg height="500" width="500">
      <path d="{}" fill="none" stroke="#000" />"
    </html>"##,
        path.to_svg()
    );
    for pt in &pts {
        println!(
            r#"      <circle cx="{}" cy="{}", r="3", fill="blue" />"#,
            pt.x, pt.y
        )
    }
    println!(
        r#"    </svg>
    </body>"#
    );
}
