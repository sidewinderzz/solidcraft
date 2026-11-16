// Procedural 3D wood grain: the GPU twin of `solidcraft_viewport::wood::sample` (wood.rs).
// Keep the two in step: same hash, same constants, same order of operations.
fn wood_hash(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    var h = (bitcast<u32>(x) * 0x8da6b343u) ^ (bitcast<u32>(y) * 0xd8163841u) ^ (bitcast<u32>(z) * 0xcb1ab31fu) ^ seed;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) / 16777216.0;
}

fn wood_noise(p: vec3<f32>, seed: u32) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (vec3<f32>(3.0) - 2.0 * f);
    let x = i32(i.x);
    let y = i32(i.y);
    let z = i32(i.z);
    let x00 = mix(wood_hash(x, y, z, seed), wood_hash(x + 1, y, z, seed), u.x);
    let x10 = mix(wood_hash(x, y + 1, z, seed), wood_hash(x + 1, y + 1, z, seed), u.x);
    let x01 = mix(wood_hash(x, y, z + 1, seed), wood_hash(x + 1, y, z + 1, seed), u.x);
    let x11 = mix(wood_hash(x, y + 1, z + 1, seed), wood_hash(x + 1, y + 1, z + 1, seed), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

fn wood_scaled(g: vec3<f32>, along: f32, across: f32) -> vec3<f32> {
    return vec3<f32>(g.x * along, g.y * across, g.z * across);
}

fn wood_smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn wood_grain(slot: u32, p: vec3<f32>) -> vec3<f32> {
    let q = vec4<f32>(p, 1.0);
    return vec3<f32>(dot(wood_texel(slot, 5u), q), dot(wood_texel(slot, 6u), q), dot(wood_texel(slot, 7u), q));
}

// A world step in grain space (the grain map without its offset).
fn wood_step(slot: u32, v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(wood_texel(slot, 5u).xyz, v), dot(wood_texel(slot, 6u).xyz, v), dot(wood_texel(slot, 7u).xyz, v));
}

fn wood_sample(slot: u32, g_in: vec3<f32>, fx_in: vec3<f32>, fy_in: vec3<f32>) -> vec4<f32> {
    let early = wood_texel(slot, 0u);
    let late_c = wood_texel(slot, 1u);
    let streak_c = wood_texel(slot, 2u);
    let k = wood_texel(slot, 3u);
    let s = u32(wood_texel(slot, 4u).x);
    let stock = wood_texel(slot, 8u);
    let ply = wood_texel(slot, 10u);
    let wild = wood_texel(slot, 9u);
    let contrast = late_c.w;
    let figure_mm = streak_c.w;
    var fx = fx_in;
    var fy = fy_in;
    var across = max(sqrt(fx.y * fx.y + fx.z * fx.z), sqrt(fy.y * fy.y + fy.z * fy.z));
    var along = max(abs(fx.x), abs(fy.x));
    var g = g_in;
    var seam = 0.0;
    var tone = 1.0;
    var grew = wood_hash(0, 0, 9, s);
    var end = 0.0;
    if (ply.x > 0.0) {
        let tp = ply.x;
        let top = max(ply.z - 1.0, 0.0);
        let zb = g.z - ply.y;
        let i = clamp(floor(zb / tp), 0.0, top);
        let ii = i32(i);
        let zl = zb - (i + 0.5) * tp;
        let turned = (ii & 1) == 1;
        var edge = 0.5 * tp - abs(zl);
        if ((i <= 0.0 && zl < 0.0) || (i >= top && zl > 0.0)) { edge = 1.0e9; }
        let fz = max(abs(fx.z), abs(fy.z));
        let line_mm = 0.1 + fz;
        seam = 0.5 * (1.0 - wood_smooth(0.0, line_mm, edge)) * (0.1 / line_mm);
        let n = vec3<f32>(fx.y * fy.z - fx.z * fy.y, fx.z * fy.x - fx.x * fy.z, fx.x * fy.y - fx.y * fy.x);
        let nn = n.x * n.x + n.y * n.y + n.z * n.z;
        var na = n.x;
        if (turned) { na = n.y; end = 0.6; }
        if (nn > 1.0e-20) { end = na * na / nn; }
        end = mix(0.5, end, 1.0 - wood_smooth(0.5, 1.5, fz / tp));
        let h1 = wood_hash(ii, 1, 0, s ^ 0x7f4a7c15u);
        let h2 = wood_hash(ii, 2, 0, s ^ 0x7f4a7c15u);
        let h3 = wood_hash(ii, 3, 0, s ^ 0x7f4a7c15u);
        let h4 = wood_hash(ii, 4, 0, s ^ 0x7f4a7c15u);
        tone = 0.95 + 0.1 * h4;
        let deep = zl + 2500.0 + 1500.0 * h3;
        if (turned) {
            g = vec3<f32>(g.y + 40000.0 * h1, g.x + 400.0 * (h2 - 0.5), deep);
            fx = vec3<f32>(fx.y, fx.x, fx.z);
            fy = vec3<f32>(fy.y, fy.x, fy.z);
            across = max(sqrt(fx.y * fx.y + fx.z * fx.z), sqrt(fy.y * fy.y + fy.z * fy.z));
            along = max(abs(fx.x), abs(fy.x));
        } else {
            g = vec3<f32>(g.x + 40000.0 * h1, g.y + 400.0 * (h2 - 0.5), deep);
        }
    } else if (stock.x > 0.0) {
        let wide = stock.x;
        let yb = g.y + stock.z;
        let j = floor(yb / wide + 0.5);
        let ji = i32(j);
        var ii = 0;
        var butt = 1.0e9;
        if (stock.y > 0.0) {
            let xs = g.x + wood_hash(ji, 0, 0, s ^ 0x2545f491u) * stock.y;
            let i = floor(xs / stock.y);
            let u = xs - i * stock.y;
            butt = min(u, stock.y - u);
            ii = i32(i);
        }
        let h1 = wood_hash(ji, ii, 1, s ^ 0x2545f491u);
        let h2 = wood_hash(ji, ii, 2, s ^ 0x2545f491u);
        let h3 = wood_hash(ji, ii, 3, s ^ 0x2545f491u);
        let h4 = wood_hash(ji, ii, 4, s ^ 0x2545f491u);
        let yl = yb - j * wide;
        let edge = min(0.5 * wide - abs(yl), butt);
        let line_mm = 0.15 + max(across, along);
        seam = 0.45 * (1.0 - wood_smooth(0.0, line_mm, edge)) * (0.15 / line_mm);
        tone = 0.93 + 0.14 * h4;
        grew = wood_hash(ji, ii, 5, s ^ 0x2545f491u);
        var side = 1.0;
        if (h3 < 0.5) { side = -1.0; }
        g = vec3<f32>(g.x + 40000.0 * h3, yl - stock.z - (h1 - 0.5) * wide * 1.2, g.z - side * (15.0 + 120.0 * h2));
    }
    let vary = clamp(wild.x, 0.0, 2.0);
    let ring_mm = early.w * mix(1.0, 0.7 + 0.7 * grew, min(vary, 1.0));
    var lift = 0.0;
    var knot = 0.0;
    var kd = 0.0;
    if (wild.y > 0.0) {
        let cell = floor(g.x / 300.0);
        let ci = i32(cell);
        if (wood_hash(ci, 1, 11, s ^ 0x7f4a7c15u) < wild.y) {
            let h1 = wood_hash(ci, 2, 11, s ^ 0x7f4a7c15u);
            let h2 = wood_hash(ci, 3, 11, s ^ 0x7f4a7c15u);
            let h3 = wood_hash(ci, 4, 11, s ^ 0x7f4a7c15u);
            let h4 = wood_hash(ci, 5, 11, s ^ 0x7f4a7c15u);
            let ay = 2.0 * h2 - 1.0;
            var az = sqrt(max(1.0 - ay * ay, 0.0));
            if (h3 < 0.5) { az = -az; }
            let out = g.y * ay + g.z * az;
            let side = g.z * ay - g.y * az;
            let u = g.x - (cell + 0.3 + 0.4 * h1) * 300.0;
            let rk = (3.0 + 9.0 * h4) * (0.4 + 0.6 * wood_smooth(0.0, 120.0, out));
            let there = wood_smooth(0.0, 8.0, out);
            kd = sqrt(u * u + side * side) / rk;
            let far = sqrt(u * u * 0.36 + side * side) / rk;
            lift = there * 1.3 * rk * (1.0 - wood_smooth(0.3, 4.5, far));
            knot = there * (1.0 - wood_smooth(0.85, 1.05, kd));
        }
    }
    if (wild.z > 0.0) {
        let b1 = g / 34.0;
        let b2 = g / 12.0;
        let sy = 22.0 * (wood_noise(b1, s ^ 0x3243f6a8u) - 0.5) + 7.0 * (wood_noise(b2, s ^ 0x85a308d3u) - 0.5);
        let sz = 22.0 * (wood_noise(vec3<f32>(b1.x + 19.0, b1.y + 7.0, b1.z + 43.0), s ^ 0x13198a2eu) - 0.5) + 7.0 * (wood_noise(vec3<f32>(b2.x + 13.0, b2.y + 37.0, b2.z + 5.0), s ^ 0x03707344u) - 0.5);
        g = vec3<f32>(g.x, g.y + wild.z * sy, g.z + wild.z * sz);
        let e = floor(g / 30.0);
        let ex = i32(e.x);
        let ey = i32(e.y);
        let ez = i32(e.z);
        if (wood_hash(ex, ey, ez, s ^ 0xa4093822u) < 0.35 + 0.6 * wild.z) {
            let c = vec3<f32>(0.35 + 0.3 * wood_hash(ex, ey, ez, s ^ 0x299f31d0u), 0.35 + 0.3 * wood_hash(ex, ey, ez, s ^ 0x082efa98u), 0.35 + 0.3 * wood_hash(ex, ey, ez, s ^ 0xec4e6c89u));
            let re = 1.2 + 2.2 * wood_hash(ex, ey, ez, s ^ 0x452821e6u);
            let d = g - (e + c) * 30.0;
            let ed = sqrt(d.x * d.x + d.y * d.y + d.z * d.z) / re;
            let eye = min(wild.z, 1.0) * (1.0 - wood_smooth(0.0, 3.0, ed));
            lift = lift + 3.0 * re * eye;
            knot = max(knot, min(wild.z, 1.0) * (1.0 - wood_smooth(0.5, 1.0, ed)));
            if (ed < 1.0) { kd = ed; }
        }
    }
    let w = wood_scaled(g, 0.004, 0.02);
    let dy = wood_noise(w, s) - 0.5;
    let dz = wood_noise(vec3<f32>(w.x + 17.0, w.y + 31.0, w.z + 47.0), s ^ 0x9e3779b9u) - 0.5;
    let w2 = wood_scaled(g, 0.0012, 0.006);
    let ey = wood_noise(vec3<f32>(w2.x + 5.0, w2.y + 11.0, w2.z + 23.0), s ^ 0x510e527fu) - 0.5;
    let ez = wood_noise(vec3<f32>(w2.x + 41.0, w2.y + 3.0, w2.z + 29.0), s ^ 0x9b05688cu) - 0.5;
    let y = g.y + figure_mm * (2.0 * dy + 9.0 * ey);
    let z = g.z + figure_mm * (2.0 * dz + 9.0 * ez);
    let r = sqrt(y * y + z * z);
    let fr = (abs(y * fx.y + z * fx.z) + abs(y * fy.y + z * fy.z)) / (max(r, 1.0e-3) * ring_mm);
    let rings = 1.0 - wood_smooth(0.35, 1.0, fr);
    let thin = 1.0 - wood_smooth(0.08, 0.35, fr);
    let uy = y / (r + 1.0e-3);
    let uz = z / (r + 1.0e-3);
    let lobe = wood_noise(vec3<f32>(g.x * 0.0015 + 3.0, 1.3 * uy + 7.0, 1.3 * uz + 13.0), s ^ 0x243f6a88u) - 0.5;
    let lobe2 = wood_noise(vec3<f32>(g.x * 0.004 + 11.0, 3.1 * uy + 5.0, 3.1 * uz + 29.0), s ^ 0xb7e15162u) - 0.5;
    let rr = r * (1.0 + vary * (0.22 * lobe + 0.07 * lobe2 * wood_smooth(5.0, 60.0, r))) + vary * 0.006 * g.x - lift;
    let ring0 = rr / ring_mm;
    let decade = wood_noise(vec3<f32>(ring0 * 0.11 + 3.0, g.x * 0.0008, 2.0 * uy + 2.0 * uz), s ^ 0x13c6ef37u) - 0.5;
    let year = wood_noise(vec3<f32>(ring0 * 0.37 + 9.0, g.x * 0.0015, 3.0 * uy - 3.0 * uz), s ^ 0x5be0cd19u) - 0.5;
    let ring = ring0
        + vary * (3.6 * decade + 0.7 * year)
        + 0.6 * (wood_noise(wood_scaled(vec3<f32>(g.x, y, z), 0.01, 0.04), s ^ 0x85ebca6bu) - 0.5)
        + 0.25 * vary * (wood_noise(wood_scaled(vec3<f32>(g.x, y, z), 0.03, 0.12), s ^ 0x1b873593u) - 0.5);
    let jitter = 0.6 * min(vary, 1.5);
    let n0 = floor(ring);
    let ni = i32(n0);
    let e0 = n0 + jitter * (wood_hash(ni, 0, 7, s ^ 0x6c62272eu) - 0.5);
    let e1 = (n0 + 1.0) + jitter * (wood_hash(ni + 1, 0, 7, s ^ 0x6c62272eu) - 0.5);
    var lo = e0;
    var hi = e1;
    var nr = ni;
    if (ring < e0) {
        lo = (n0 - 1.0) + jitter * (wood_hash(ni - 1, 0, 7, s ^ 0x6c62272eu) - 0.5);
        hi = e0;
        nr = ni - 1;
    } else if (ring >= e1) {
        lo = e1;
        hi = (n0 + 2.0) + jitter * (wood_hash(ni + 2, 0, 7, s ^ 0x6c62272eu) - 0.5);
        nr = ni + 1;
    }
    let t = clamp((ring - lo) / (hi - lo), 0.0, 1.0);
    let share = mix(0.55, 0.3 + 0.5 * wood_hash(nr, 1, 7, s ^ 0x6c62272eu), min(vary, 1.0));
    let depth = mix(1.0, 0.5 + 0.7 * wood_hash(nr, 2, 7, s ^ 0x6c62272eu), min(vary, 1.0));
    let wrap = 1.0 - wood_smooth(1.0 - clamp(fr, 0.02, 0.5), 1.0, t);
    let late = mix(0.25, min(wood_smooth(share, 0.95, t) * wrap * depth, 1.0), rings) * contrast;
    let q = vec3<f32>(g.x, y, z);
    let light = wood_texel(slot, 4u).yzw;
    let around = 20.0 * y / (r + 1.0);
    let zone = wood_smooth(0.35, 0.8, wood_noise(vec3<f32>(g.x * 0.0015, r * 0.05, around * 0.05), s ^ 0x165667b1u));
    let streak = wood_smooth(0.6, 0.92, wood_noise(vec3<f32>(g.x * 0.004, r * 0.25, around * 0.03), s ^ 0xc2b2ae35u)) * k.y;
    let fine = 1.0 - wood_smooth(0.08, 0.25, across);
    let coarse = 1.0 - wood_smooth(0.25, 0.8, across);
    let mid = 1.0 - wood_smooth(0.8, 2.5, across);
    let pore = wood_smooth(1.0 - 0.3 * k.x, 1.0, wood_noise(wood_scaled(q, 0.1, 1.8), s ^ 0x27d4eb2fu)) * min(k.x, 1.0) * (1.0 - 0.5 * late) * coarse * (1.0 - wood_smooth(3.0, 10.0, along));
    let fleck = wood_smooth(0.8, 0.97, wood_noise(wood_scaled(q, 0.3, 1.6), s ^ 0x6a09e667u)) * (0.25 + 0.5 * k.x) * coarse * (1.0 - wood_smooth(1.0, 3.3, along));
    let fibre = mix(1.0, 0.92 + 0.16 * wood_noise(wood_scaled(q, 0.03, 1.2), s ^ 0x3c6ef372u), coarse)
        * mix(1.0, 0.94 + 0.12 * wood_noise(wood_scaled(q, 0.05, 4.0), s ^ 0xbb67ae85u), fine)
        * mix(1.0, 0.95 + 0.1 * wood_noise(wood_scaled(q, 0.012, 0.45), s ^ 0xa54ff53au), mid);
    let line = wood_smooth(0.88, 0.97, t) * wrap * thin * contrast * min(depth, 1.0);
    let ripple = mix(1.0, 0.95 + 0.1 * wood_noise(wood_scaled(g, 0.06, 0.012), s ^ 0x1f83d9abu), 1.0 - wood_smooth(5.0, 16.0, along));
    let base = mix(mix(early.xyz, light, zone), late_c.xyz, late);
    let streaked = mix(base, streak_c.xyz, streak);
    let knot_ring = max(wood_smooth(0.5, 1.0, abs(kd * 4.0 - floor(kd * 4.0) - 0.5) * 2.0), wood_smooth(0.7, 1.0, kd));
    let flecked0 = mix(streaked, light, 0.35 * fleck);
    let flecked = mix(flecked0, late_c.xyz * mix(streak_c.xyz, vec3<f32>(1.0), 0.45), knot * (0.75 + 0.25 * knot_ring));
    let c = clamp(flecked * (1.0 - 0.7 * pore) * (1.0 - 0.2 * line) * fibre * ripple * tone * (1.0 - seam) * mix(vec3<f32>(1.0), vec3<f32>(0.80, 0.72, 0.62), end), vec3<f32>(0.0), vec3<f32>(1.0));
    let roughness = clamp(mix(k.z, k.w, late / max(contrast, 1e-3)) + 0.08 * pore, 0.02, 1.0);
    return vec4<f32>(c, roughness);
}
