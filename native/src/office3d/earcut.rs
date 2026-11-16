//! Earcut as three.js 0.170 runs it (ShapeUtils.triangulateShape), for the
//! one kind of outline the office fills: a convex polygon with no holes and
//! under 80 points, so no z-order hashing. The ears come off in the same order
//! as in JavaScript, so the lids get the same triangles.

/// Triangles as index triples into `pts`.
pub fn triangulate(pts: &[(f64, f64)]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    // linkedList(data, 0, len, dim, clockwise = true): kept in order when the
    // signed area says so, else walked backwards.
    let mut sum = 0.0;
    let mut j = n - 1;
    for i in 0..n {
        sum += (pts[j].0 - pts[i].0) * (pts[i].1 + pts[j].1);
        j = i;
    }
    let order: Vec<usize> = if sum > 0.0 { (0..n).collect() } else { (0..n).rev().collect() };
    // The ring as prev/next links over `order`; the last node inserted is the start.
    let m = order.len();
    let mut prev: Vec<usize> = (0..m).map(|i| (i + m - 1) % m).collect();
    let mut next: Vec<usize> = (0..m).map(|i| (i + 1) % m).collect();
    let mut alive = vec![true; m];
    let p = |k: usize| pts[order[k]];
    // A repeated last point goes, as linkedList drops it.
    let mut ear = m - 1;
    if p(ear) == p(next[ear]) {
        let (a, b) = (prev[ear], next[ear]);
        next[a] = b;
        prev[b] = a;
        alive[ear] = false;
        ear = b;
    }
    let area = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| (b.1 - a.1) * (c.0 - b.0) - (b.0 - a.0) * (c.1 - b.1);
    let inside = |a: (f64, f64), b: (f64, f64), c: (f64, f64), q: (f64, f64)| {
        (c.0 - q.0) * (a.1 - q.1) >= (a.0 - q.0) * (c.1 - q.1) && (a.0 - q.0) * (b.1 - q.1) >= (b.0 - q.0) * (a.1 - q.1) && (b.0 - q.0) * (c.1 - q.1) >= (c.0 - q.0) * (b.1 - q.1)
    };
    let mut out = Vec::new();
    let mut stop = ear;
    let mut guard = 0;
    while prev[ear] != next[ear] && guard < m * m {
        guard += 1;
        let (a, b) = (prev[ear], next[ear]);
        // isEar: not reflex, and no other point of the ring inside it.
        let is_ear = area(p(a), p(ear), p(b)) < 0.0 && {
            let mut k = next[b];
            let mut clear = true;
            while k != a {
                if inside(p(a), p(ear), p(b), p(k)) && area(p(prev[k]), p(k), p(next[k])) >= 0.0 {
                    clear = false;
                    break;
                }
                k = next[k];
            }
            clear
        };
        if is_ear {
            out.push([order[a], order[ear], order[b]]);
            next[a] = b;
            prev[b] = a;
            alive[ear] = false;
            ear = next[b];
            stop = next[b];
            continue;
        }
        ear = b;
        if ear == stop {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_square_is_two_triangles_covering_it() {
        let t = super::triangulate(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]);
        assert_eq!(t.len(), 2);
    }
}
