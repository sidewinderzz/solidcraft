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

fn wood_sample(slot: u32, g: vec3<f32>, fp: f32) -> vec4<f32> {
    let early = wood_texel(slot, 0u);
    let late_c = wood_texel(slot, 1u);
    let streak_c = wood_texel(slot, 2u);
    let k = wood_texel(slot, 3u);
    let s = u32(wood_texel(slot, 4u).x);
    let ring_mm = early.w;
    let contrast = late_c.w;
    let figure_mm = streak_c.w;
    let w = wood_scaled(g, 0.004, 0.02);
    let dy = wood_noise(w, s) - 0.5;
    let dz = wood_noise(vec3<f32>(w.x + 17.0, w.y + 31.0, w.z + 47.0), s ^ 0x9e3779b9u) - 0.5;
    let rings = 1.0 - wood_smooth(0.6, 1.4, fp / ring_mm);
    let fine = 1.0 - wood_smooth(1.2, 4.0, fp);
    let w2 = wood_scaled(g, 0.0012, 0.006);
    let ey = wood_noise(vec3<f32>(w2.x + 5.0, w2.y + 11.0, w2.z + 23.0), s ^ 0x510e527fu) - 0.5;
    let ez = wood_noise(vec3<f32>(w2.x + 41.0, w2.y + 3.0, w2.z + 29.0), s ^ 0x9b05688cu) - 0.5;
    let y = g.y + figure_mm * (2.0 * dy + 9.0 * ey);
    let z = g.z + figure_mm * (2.0 * dz + 9.0 * ez);
    let r = sqrt(y * y + z * z);
    let ring = r / ring_mm + 0.6 * (wood_noise(wood_scaled(vec3<f32>(g.x, y, z), 0.01, 0.04), s ^ 0x85ebca6bu) - 0.5);
    let t = ring - floor(ring);
    let late = mix(0.25, wood_smooth(0.55, 0.95, t), rings) * contrast;
    let q = vec3<f32>(g.x, y, z);
    let light = wood_texel(slot, 4u).yzw;
    let around = 20.0 * y / (r + 1.0);
    let zone = wood_smooth(0.35, 0.8, wood_noise(vec3<f32>(g.x * 0.0015, r * 0.05, around * 0.05), s ^ 0x165667b1u));
    let streak = wood_smooth(0.6, 0.92, wood_noise(vec3<f32>(g.x * 0.004, r * 0.25, around * 0.03), s ^ 0xc2b2ae35u)) * k.y;
    let pore = wood_smooth(1.0 - 0.3 * k.x, 1.0, wood_noise(wood_scaled(q, 0.1, 1.8), s ^ 0x27d4eb2fu)) * min(k.x, 1.0) * (1.0 - 0.5 * late) * fine;
    let fleck = wood_smooth(0.8, 0.97, wood_noise(wood_scaled(q, 0.3, 1.6), s ^ 0x6a09e667u)) * (0.25 + 0.5 * k.x) * fine;
    let fibre = mix(1.0, (0.92 + 0.16 * wood_noise(wood_scaled(q, 0.03, 1.2), s ^ 0x3c6ef372u)) * (0.94 + 0.12 * wood_noise(wood_scaled(q, 0.05, 4.0), s ^ 0xbb67ae85u)), fine);
    let line = wood_smooth(0.88, 0.97, t) * rings * contrast;
    let ripple = 0.95 + 0.1 * wood_noise(wood_scaled(g, 0.06, 0.012), s ^ 0x1f83d9abu);
    let base = mix(mix(early.xyz, light, zone), late_c.xyz, late);
    let streaked = mix(base, streak_c.xyz, streak);
    let flecked = mix(streaked, light, 0.35 * fleck);
    let c = clamp(flecked * (1.0 - 0.7 * pore) * (1.0 - 0.25 * line) * fibre * ripple, vec3<f32>(0.0), vec3<f32>(1.0));
    let roughness = clamp(mix(k.z, k.w, late / max(contrast, 1e-3)) + 0.08 * pore, 0.02, 1.0);
    return vec4<f32>(c, roughness);
}
