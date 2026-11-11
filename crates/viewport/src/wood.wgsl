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

fn wood_sample(slot: u32, g_in: vec3<f32>, fx: vec3<f32>, fy: vec3<f32>) -> vec4<f32> {
    let early = wood_texel(slot, 0u);
    let late_c = wood_texel(slot, 1u);
    let streak_c = wood_texel(slot, 2u);
    let k = wood_texel(slot, 3u);
    let s = u32(wood_texel(slot, 4u).x);
    let stock = wood_texel(slot, 8u);
    let ring_mm = early.w;
    let contrast = late_c.w;
    let figure_mm = streak_c.w;
    let across = max(sqrt(fx.y * fx.y + fx.z * fx.z), sqrt(fy.y * fy.y + fy.z * fy.z));
    let along = max(abs(fx.x), abs(fy.x));
    var g = g_in;
    var seam = 0.0;
    var tone = 1.0;
    if (stock.x > 0.0) {
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
        var side = 1.0;
        if (h3 < 0.5) { side = -1.0; }
        g = vec3<f32>(g.x + 40000.0 * h3, yl - stock.z - (h1 - 0.5) * wide * 1.2, g.z - side * (15.0 + 120.0 * h2));
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
    let ring = r / ring_mm + 0.6 * (wood_noise(wood_scaled(vec3<f32>(g.x, y, z), 0.01, 0.04), s ^ 0x85ebca6bu) - 0.5);
    let t = ring - floor(ring);
    let wrap = 1.0 - wood_smooth(1.0 - clamp(fr, 0.02, 0.5), 1.0, t);
    let late = mix(0.25, wood_smooth(0.55, 0.95, t) * wrap, rings) * contrast;
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
    let line = wood_smooth(0.88, 0.97, t) * wrap * thin * contrast;
    let ripple = mix(1.0, 0.95 + 0.1 * wood_noise(wood_scaled(g, 0.06, 0.012), s ^ 0x1f83d9abu), 1.0 - wood_smooth(5.0, 16.0, along));
    let base = mix(mix(early.xyz, light, zone), late_c.xyz, late);
    let streaked = mix(base, streak_c.xyz, streak);
    let flecked = mix(streaked, light, 0.35 * fleck);
    let c = clamp(flecked * (1.0 - 0.7 * pore) * (1.0 - 0.2 * line) * fibre * ripple * tone * (1.0 - seam), vec3<f32>(0.0), vec3<f32>(1.0));
    let roughness = clamp(mix(k.z, k.w, late / max(contrast, 1e-3)) + 0.08 * pore, 0.02, 1.0);
    return vec4<f32>(c, roughness);
}
