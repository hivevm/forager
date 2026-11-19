//! The bits of `smoothsia`'s results on 20,000 random parts, one line a case: the probe of ticket
//! 02 of revision-2026-11-19, which made the window walk cheaper and kept every result's bits.
//! Each case is a line of 1 to 40 points, a closed ring in about half of them, smoothed by a
//! window of 1 to 45 points, over 1 to 4 iterations, with no preprocessing, `all`, or `angle`.
//! The coordinates are ordinary, huge, tiny, repeated, or NaN or infinite. The cases come from a
//! fixed seed, so two versions of the crate print the same cases, and their outputs compare by
//! `cmp`. `crates/symbolize/smoothsia-bits.sh <commit>` runs it on a commit and on the tree.
//!
//! Each line holds the case's number and the bits of every coordinate of its result, or the
//! reason the transform drew nothing.

use std::fmt::Write as _;
use std::io::Write as _;

use forager_style_ir::{Coord, Geometry, Preprocessing, TransformNumber, TransformShape};
use forager_symbolize::{Budget, transformed};

/// How many cases the probe smooths.
const CASES: usize = 20_000;

/// A xorshift64 generator: the probe needs no dependency, and its cases no quality.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A number below `n`.
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn unit(&mut self) -> f64 {
        // 53 bits of the draw, as a fraction of 1.
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A coordinate: mostly within a few hundred pixels, sometimes far out, tiny, or no number.
    fn coordinate(&mut self) -> f64 {
        match self.below(40) {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => f64::NEG_INFINITY,
            3 => self.unit() * 1e300,
            4 => self.unit() * 1e-300,
            5 => 0.0,
            _ => (self.unit() - 0.5) * 1000.0,
        }
    }

    fn coord(&mut self) -> Coord {
        Coord {
            x: self.coordinate(),
            y: self.coordinate(),
        }
    }
}

fn main() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut out = String::new();
    for case in 0..CASES {
        let n = usize::try_from(1 + rng.below(40)).unwrap_or(1);
        let mut line: Vec<Coord> = Vec::with_capacity(n + 1);
        for _ in 0..n {
            // A point repeated now and then, as a ring's first point is at its end.
            if !line.is_empty() && rng.below(8) == 0 {
                let k = usize::try_from(rng.below(line.len() as u64)).unwrap_or(0);
                line.push(line[k]);
            } else {
                line.push(rng.coord());
            }
        }
        if n >= 2 && rng.below(2) == 0 {
            line.push(line[0]);
        }
        let size = (1 + rng.below(45)) as f64;
        let iterations = (1 + rng.below(4)) as f64;
        let preprocessing = match rng.below(3) {
            0 => None,
            1 => Some(Preprocessing::All),
            _ => Some(Preprocessing::Angle),
        };
        let shape = TransformShape::SmoothSia {
            shape: Box::new(TransformShape::Shape),
            size: TransformNumber::Number(size),
            iterations: TransformNumber::Number(iterations),
            preprocessing,
        };
        let geometry = Geometry::MultiLineString(vec![line]);
        let _ = write!(out, "{case}");
        match transformed(&shape, &geometry, 1.0, &mut Budget::default()) {
            Ok(Geometry::MultiLineString(parts)) => {
                for part in parts {
                    out.push_str(" |");
                    for p in part {
                        let _ = write!(out, " {:x},{:x}", p.x.to_bits(), p.y.to_bits());
                    }
                }
            }
            Ok(Geometry::LineString(part)) => {
                for p in part {
                    let _ = write!(out, " {:x},{:x}", p.x.to_bits(), p.y.to_bits());
                }
            }
            Ok(other) => {
                let _ = write!(out, " {other:?}");
            }
            Err(why) => {
                let _ = write!(out, " {why:?}");
            }
        }
        out.push('\n');
    }
    std::io::stdout()
        .write_all(out.as_bytes())
        .expect("the cases are written to stdout");
}
