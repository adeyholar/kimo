//! Arbitrary vector paths (SVG path syntax: M L H V C S Q T Z, absolute and relative; arcs are rejected loudly) flattened to
//! polylines. Gives a signed distance (nonzero or even-odd fill) and an exact arc-length parameter, so shape modifiers
//! (trim, offset, merge, repeater, gradient stroke) work on any path. Coordinates are pixels relative to the canvas center.
pub const MAX_SEGMENTS: usize = 6000;
#[derive(Clone, Debug)]
pub struct Path { pub segs: Vec<[f64; 4]>, pub cum: Vec<f64>, pub total: f64, pub bbox: [f64; 4], pub evenodd: bool, pub subpaths: usize }

fn tokens(d: &str) -> Result<Vec<(char, Vec<f64>)>, String> {
    let b: Vec<char> = d.chars().collect(); let (mut i, mut out): (usize, Vec<(char, Vec<f64>)>) = (0, vec![]);
    while i < b.len() { let c = b[i];
        if c.is_whitespace() || c == ',' { i += 1; continue; }
        if c.is_ascii_alphabetic() { if !"MmLlHhVvCcSsQqTtZz".contains(c) { return Err(if "Aa".contains(c) { "arc commands (A) are not supported; use C/Q curves or points+smooth".into() } else { format!("unknown path command '{c}'") }); } out.push((c, vec![])); i += 1; continue; }
        let st = i; if b[i] == '+' || b[i] == '-' { i += 1; } let mut dot = false; while i < b.len() && (b[i].is_ascii_digit() || (b[i] == '.' && !dot)) { if b[i] == '.' { dot = true; } i += 1; }
        if i < b.len() && (b[i] == 'e' || b[i] == 'E') { i += 1; if i < b.len() && (b[i] == '+' || b[i] == '-') { i += 1; } while i < b.len() && b[i].is_ascii_digit() { i += 1; } }
        let s: String = b[st..i].iter().collect(); let v: f64 = s.parse().map_err(|_| format!("bad number '{s}' at char {st}"))?; if !v.is_finite() || v.abs() > 1e6 { return Err(format!("number {v} out of range")); }
        match out.last_mut() { Some((_, a)) => a.push(v), None => return Err("path must start with a command (M)".into()) } }
    Ok(out)
}

impl Path {
    pub fn parse(d: &str, evenodd: bool) -> Result<Path, String> {
        let toks = tokens(d)?; if toks.is_empty() { return Err("empty path".into()); }
        if toks[0].0.to_ascii_uppercase() != 'M' { return Err("path must start with M (moveto)".into()); }
        let mut segs: Vec<[f64; 4]> = vec![]; let (mut cx, mut cy, mut sx, mut sy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64); let (mut lcx, mut lcy) = (0.0f64, 0.0f64); let mut last = ' '; let mut subs = 0usize; let mut open = false;
        let mut line = |segs: &mut Vec<[f64; 4]>, x0: f64, y0: f64, x1: f64, y1: f64| segs.push([x0, y0, x1, y1]);
        macro_rules! close { () => { if open && ((cx - sx).abs() > 1e-9 || (cy - sy).abs() > 1e-9) { line(&mut segs, cx, cy, sx, sy); } cx = sx; cy = sy; open = false; } }
        for (cmd, a) in &toks {
            let rel = cmd.is_lowercase(); let up = cmd.to_ascii_uppercase();
            let need = match up { 'M' | 'L' | 'T' => 2, 'H' | 'V' => 1, 'C' => 6, 'S' | 'Q' => 4, _ => 0 };
            if up == 'Z' { if !a.is_empty() { return Err("Z takes no numbers".into()); } close!(); last = 'Z'; continue; }
            if a.is_empty() || a.len() % need != 0 { return Err(format!("command '{cmd}' needs groups of {need} numbers, got {}", a.len())); }
            for (gi, g) in a.chunks(need).enumerate() {
                let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };
                match up {
                    'M' => { if gi == 0 { close!(); } let (x, y) = (g[0] + ox, g[1] + oy); if gi == 0 { sx = x; sy = y; cx = x; cy = y; subs += 1; open = true; } else { line(&mut segs, cx, cy, x, y); cx = x; cy = y; } last = if gi == 0 { 'M' } else { 'L' }; }
                    'L' => { let (x, y) = (g[0] + ox, g[1] + oy); line(&mut segs, cx, cy, x, y); cx = x; cy = y; last = 'L'; }
                    'H' => { let x = g[0] + ox; line(&mut segs, cx, cy, x, cy); cx = x; last = 'L'; }
                    'V' => { let y = g[0] + oy; line(&mut segs, cx, cy, cx, y); cy = y; last = 'L'; }
                    'C' | 'S' | 'Q' | 'T' => {
                        let (p0, p3, c1, c2): ([f64; 2], [f64; 2], [f64; 2], [f64; 2]);
                        p0 = [cx, cy];
                        match up {
                            'C' => { c1 = [g[0] + ox, g[1] + oy]; c2 = [g[2] + ox, g[3] + oy]; p3 = [g[4] + ox, g[5] + oy]; }
                            'S' => { c1 = if last == 'C' { [2.0 * cx - lcx, 2.0 * cy - lcy] } else { [cx, cy] }; c2 = [g[0] + ox, g[1] + oy]; p3 = [g[2] + ox, g[3] + oy]; }
                            'Q' => { let q = [g[0] + ox, g[1] + oy]; p3 = [g[2] + ox, g[3] + oy]; c1 = [cx + 2.0 / 3.0 * (q[0] - cx), cy + 2.0 / 3.0 * (q[1] - cy)]; c2 = [p3[0] + 2.0 / 3.0 * (q[0] - p3[0]), p3[1] + 2.0 / 3.0 * (q[1] - p3[1])]; lcx = q[0]; lcy = q[1]; }
                            _ => { let q = if last == 'Q' { [2.0 * cx - lcx, 2.0 * cy - lcy] } else { [cx, cy] }; p3 = [g[0] + ox, g[1] + oy]; c1 = [cx + 2.0 / 3.0 * (q[0] - cx), cy + 2.0 / 3.0 * (q[1] - cy)]; c2 = [p3[0] + 2.0 / 3.0 * (q[0] - p3[0]), p3[1] + 2.0 / 3.0 * (q[1] - p3[1])]; lcx = q[0]; lcy = q[1]; }
                        }
                        if up == 'C' || up == 'S' { lcx = c2[0]; lcy = c2[1]; }
                        let cl = ((c1[0] - p0[0]).hypot(c1[1] - p0[1])) + ((c2[0] - c1[0]).hypot(c2[1] - c1[1])) + ((p3[0] - c2[0]).hypot(p3[1] - c2[1]));
                        let n = ((cl / 3.0).ceil() as usize).clamp(8, 96); let mut pv = p0;
                        for k in 1..=n { let t = k as f64 / n as f64; let u = 1.0 - t; let q = [0, 1].map(|j| u * u * u * p0[j] + 3.0 * u * u * t * c1[j] + 3.0 * u * t * t * c2[j] + t * t * t * p3[j]); line(&mut segs, pv[0], pv[1], q[0], q[1]); pv = q; }
                        cx = p3[0]; cy = p3[1]; last = if matches!(up, 'C' | 'S') { 'C' } else { 'Q' }; }
                    _ => {}
                }
                if segs.len() > MAX_SEGMENTS { return Err(format!("path too complex (> {MAX_SEGMENTS} flattened segments)")); }
            }
        }
        close!(); segs.retain(|s| (s[2] - s[0]).hypot(s[3] - s[1]) > 1e-12);
        if segs.is_empty() { return Err("path has no drawable segments".into()); }
        let (mut cum, mut tot) = (Vec::with_capacity(segs.len()), 0.0); let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for s in &segs { cum.push(tot); tot += (s[2] - s[0]).hypot(s[3] - s[1]); for (x, y) in [(s[0], s[1]), (s[2], s[3])] { bb[0] = bb[0].min(x); bb[1] = bb[1].min(y); bb[2] = bb[2].max(x); bb[3] = bb[3].max(y); } }
        Ok(Path { segs, cum, total: tot, bbox: bb, evenodd, subpaths: subs })
    }
    /// Unsigned distance and arc-length fraction (0..1 over the whole path) of the nearest point.
    pub fn nearest(&self, px: f64, py: f64) -> (f64, f64) {
        let (mut best, mut bu) = (f64::MAX, 0.0);
        for (i, s) in self.segs.iter().enumerate() { let (dx, dy) = (s[2] - s[0], s[3] - s[1]); let l2 = dx * dx + dy * dy; let t = (((px - s[0]) * dx + (py - s[1]) * dy) / l2).clamp(0.0, 1.0);
            let (qx, qy) = (s[0] + dx * t, s[1] + dy * t); let d2 = (px - qx).powi(2) + (py - qy).powi(2); if d2 < best { best = d2; bu = (self.cum[i] + t * l2.sqrt()) / self.total; } }
        (best.sqrt(), bu)
    }
    pub fn winding(&self, px: f64, py: f64) -> i32 {
        let mut w = 0; for s in &self.segs { let (x0, y0, x1, y1) = (s[0], s[1], s[2], s[3]);
            if (y0 <= py) != (y1 <= py) { let t = (py - y0) / (y1 - y0); if x0 + t * (x1 - x0) > px { w += if y1 > y0 { 1 } else { -1 }; } } } w
    }
    /// Signed distance, negative inside.
    pub fn sd(&self, px: f64, py: f64) -> f64 {
        // far-field cull: beyond CULL px of the bbox the exact distance is not needed (shapes, strokes and offsets reach < CULL px)
        const CULL: f64 = 256.0; let b = &self.bbox; let (ex, ey) = ((b[0] - px).max(px - b[2]).max(0.0), (b[1] - py).max(py - b[3]).max(0.0)); let bd = ex.hypot(ey); if bd > CULL { return bd; }
        let (d, _) = self.nearest(px, py); let w = self.winding(px, py); let inside = if self.evenodd { w % 2 != 0 } else { w != 0 }; if inside { -d } else { d }
    }
}

/// Catmull-Rom through `pts` to cubic Beziers, as an SVG path string. tension 0..1 (0.5 = standard).
pub fn smooth_d(pts: &[[f64; 2]], closed: bool, tension: f64) -> String {
    let n = pts.len(); let k = tension / 3.0 * 2.0; let at = |i: isize| -> [f64; 2] { if closed { pts[i.rem_euclid(n as isize) as usize] } else { pts[i.clamp(0, n as isize - 1) as usize] } };
    let mut s = format!("M{} {}", pts[0][0], pts[0][1]); let segs = if closed { n } else { n - 1 };
    for i in 0..segs as isize { let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let c1 = [p1[0] + (p2[0] - p0[0]) * k / 2.0 * 1.0, p1[1] + (p2[1] - p0[1]) * k / 2.0]; let c2 = [p2[0] - (p3[0] - p1[0]) * k / 2.0, p2[1] - (p3[1] - p1[1]) * k / 2.0];
        s += &format!(" C{} {} {} {} {} {}", c1[0], c1[1], c2[0], c2[1], p2[0], p2[1]); }
    if closed { s += " Z"; } s
}
