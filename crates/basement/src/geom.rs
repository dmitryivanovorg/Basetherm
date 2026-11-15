//! Rectilinear polygons in interior coordinates (metres): membership, L-infinity distance to the
//! boundary, mirror images and validity checks. Polygons are vertex lists without a repeated end point.

pub type Pt = (f64, f64);

const EPS: f64 = 1e-9;

/// Whether the point is inside (even-odd rule, half-open edges so a vertex is never counted twice).
pub fn contains(p: &[Pt], x: f64, y: f64) -> bool {
    let n = p.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        if (a.0 - b.0).abs() < EPS && x < a.0 && y >= a.1.min(b.1) && y < a.1.max(b.1) {
            inside = !inside;
        }
    }
    inside
}

/// Chebyshev distance from the point to the nearest boundary edge, m.
pub fn edge_distance(p: &[Pt], x: f64, y: f64) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            let dx = (a.0.min(b.0) - x).max(x - a.0.max(b.0)).max(0.0);
            let dy = (a.1.min(b.1) - y).max(y - a.1.max(b.1)).max(0.0);
            dx.max(dy)
        })
        .fold(f64::INFINITY, f64::min)
}

pub fn signed_area(p: &[Pt]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| p[i].0 * p[(i + 1) % n].1 - p[(i + 1) % n].0 * p[i].1)
        .sum::<f64>()
        / 2.0
}

pub fn rectangle(x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<Pt> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// Bounding box `(x0, x1, y0, y1)`.
pub fn bounds(p: &[Pt]) -> (f64, f64, f64, f64) {
    p.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(x0, x1, y0, y1), q| (x0.min(q.0), x1.max(q.0), y0.min(q.1), y1.max(q.1)),
    )
}

/// The polygon counter-clockwise (interior on the left of each edge).
pub fn ccw(p: &[Pt]) -> Vec<Pt> {
    let mut v = p.to_vec();
    if signed_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

pub fn mirror(p: &[Pt], lx: f64, ly: f64, flip_x: bool, flip_y: bool) -> Vec<Pt> {
    p.iter()
        .map(|&(x, y)| {
            (
                if flip_x { lx - x } else { x },
                if flip_y { ly - y } else { y },
            )
        })
        .collect()
}

/// A form that is equal for the same shape however it is described: collinear and repeated
/// vertices dropped, counter-clockwise, starting at the lowest-left vertex, coordinates rounded to
/// a micrometre.
pub fn canonical(p: &[Pt]) -> Vec<(i64, i64)> {
    let key = |v: f64| (v * 1e6).round() as i64;
    let mut v: Vec<(i64, i64)> = ccw(p).iter().map(|q| (key(q.0), key(q.1))).collect();
    loop {
        let n = v.len();
        let drop = (0..n).find(|&i| {
            let (a, b, c) = (v[(i + n - 1) % n], v[i], v[(i + 1) % n]);
            b == a || (a.0 == b.0 && b.0 == c.0) || (a.1 == b.1 && b.1 == c.1)
        });
        match drop {
            Some(i) if n > 3 => {
                v.remove(i);
            }
            _ => break,
        }
    }
    let start = (0..v.len()).min_by_key(|&i| v[i]).unwrap_or(0);
    v.rotate_left(start);
    v
}

/// Why the vertex list is not a simple rectilinear polygon, if it is not.
pub fn problem(p: &[Pt]) -> Option<String> {
    let n = p.len();
    if n < 4 || !n.is_multiple_of(2) {
        return Some("a rectilinear polygon needs an even number of corners, at least 4".into());
    }
    if p.iter().any(|q| !(q.0.is_finite() && q.1.is_finite())) {
        return Some("polygon corners must be finite".into());
    }
    let vertical = |i: usize| (p[i].0 - p[(i + 1) % n].0).abs() < EPS;
    let horizontal = |i: usize| (p[i].1 - p[(i + 1) % n].1).abs() < EPS;
    for i in 0..n {
        let len = (p[i].0 - p[(i + 1) % n].0).abs() + (p[i].1 - p[(i + 1) % n].1).abs();
        if len < EPS || !(vertical(i) || horizontal(i)) {
            return Some("polygon edges must be horizontal or vertical and non-zero".into());
        }
        if vertical(i) == vertical((i + 1) % n) {
            return Some("polygon edges must alternate between horizontal and vertical".into());
        }
    }
    // Axis-aligned segments intersect exactly when their bounding boxes do.
    let boxed = |i: usize| {
        let (a, b) = (p[i], p[(i + 1) % n]);
        (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1))
    };
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (a, b) = (boxed(i), boxed(j));
            if a.0 <= b.1 + EPS && b.0 <= a.1 + EPS && a.2 <= b.3 + EPS && b.2 <= a.3 + EPS {
                return Some("polygon edges cross or touch".into());
            }
        }
    }
    None
}

/// Centres of the elementary rectangles spanned by the corner coordinates of both polygons.
fn elementary(a: &[Pt], b: &[Pt]) -> Vec<Pt> {
    let axis = |f: fn(&Pt) -> f64| {
        let mut c: Vec<f64> = a.iter().chain(b).map(f).collect();
        c.sort_by(f64::total_cmp);
        c.dedup_by(|p, q| (*p - *q).abs() < EPS);
        c.windows(2)
            .map(|w| (w[0] + w[1]) / 2.0)
            .collect::<Vec<_>>()
    };
    let (xs, ys) = (axis(|q| q.0), axis(|q| q.1));
    xs.iter()
        .flat_map(|&x| ys.iter().map(move |&y| (x, y)))
        .collect()
}

/// Whether the two polygons share any area.
pub fn overlap(a: &[Pt], b: &[Pt]) -> bool {
    elementary(a, b)
        .iter()
        .any(|&(x, y)| contains(a, x, y) && contains(b, x, y))
}

/// Whether `inner` lies entirely inside `outer`.
pub fn covers(outer: &[Pt], inner: &[Pt]) -> bool {
    elementary(outer, inner)
        .iter()
        .all(|&(x, y)| !contains(inner, x, y) || contains(outer, x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ell() -> Vec<Pt> {
        vec![(0., 0.), (6., 0.), (6., 3.), (3., 3.), (3., 5.), (0., 5.)]
    }

    #[test]
    fn membership_and_distance_of_an_l_shape() {
        let p = ell();
        assert!(contains(&p, 1.0, 4.0) && contains(&p, 5.0, 1.0));
        assert!(!contains(&p, 5.0, 4.0) && !contains(&p, -1.0, 1.0));
        assert!((edge_distance(&p, 5.0, 4.0) - 1.0).abs() < 1e-12);
        assert!((edge_distance(&p, 4.0, 1.5) - 1.5).abs() < 1e-12);
        assert!((edge_distance(&p, 4.0, 4.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn mirror_images_have_one_canonical_form() {
        let p = ell();
        let m = mirror(&p, 6.0, 5.0, true, true);
        assert_ne!(canonical(&p), canonical(&m));
        assert_eq!(canonical(&p), canonical(&mirror(&m, 6.0, 5.0, true, true)));
        let mut q = p.clone();
        q.reverse();
        q.rotate_left(2);
        q.insert(1, (q[0].0, (q[0].1 + q[1].1) / 2.0)); // an extra collinear corner
        assert_eq!(canonical(&p), canonical(&q));
    }

    #[test]
    fn invalid_polygons_are_explained() {
        assert!(problem(&ell()).is_none());
        assert!(problem(&[(0., 0.), (1., 1.), (0., 2.), (-1., 1.)]).is_some());
        // Bow-tie of rectilinear edges: edges cross.
        let bow = vec![(0., 0.), (4., 0.), (4., 4.), (2., 4.), (2., -2.), (0., -2.)];
        assert!(problem(&bow).is_some());
        assert!(problem(&[(0., 0.), (1., 0.), (1., 1.)]).is_some());
    }

    #[test]
    fn overlap_and_cover_work_on_l_shapes() {
        let l = ell();
        let a = rectangle(0.0, 3.0, 0.0, 5.0);
        let b = rectangle(3.0, 6.0, 0.0, 3.0);
        assert!(covers(&l, &a) && covers(&l, &b));
        assert!(!overlap(&a, &b));
        assert!(overlap(&a, &rectangle(2.0, 4.0, 1.0, 2.0)));
        assert!(!covers(&l, &rectangle(3.0, 6.0, 0.0, 5.0)));
    }
}
