//! Procedural 3D wood grain: a colour and a roughness for any point of a body, worked out from
//! where the point lies in the body's grain frame. There are no texture coordinates, so there
//! are no seams and no stretching on tapered, angled or curved faces.
//!
//! The grain frame puts the tree's pith on the frame's X axis: X runs along the grain, and the
//! growth rings are circles around X in the YZ plane. The rings are distorted with low-frequency
//! noise (the figure), and fine streaks and pores run along X.
//!
//! The same function runs on the CPU ([`WoodShade::sample`]) and on the GPU ([`WGSL`]). Both use
//! `f32`, the same integer hash and the same constants, so the viewport, the headless model
//! render and the render jobs show the same grain (to within GPU rounding).

use crate::vec::Vec3Ops;
use solidcore_record::Vec3;

/// One species' look: colours in sRGB (0..1) and the shape of its grain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WoodLook {
    /// Earlywood: the light, soft part of each ring.
    pub early: [f32; 3],
    /// Latewood: the dense, dark band at the end of each ring.
    pub late: [f32; 3],
    /// The colour of the dark streaks along the grain (walnut's near-black lines, teak's oily
    /// ones).
    pub streak: [f32; 3],
    /// The third tone: the lighter, warmer colour of the broad zones that run along a board,
    /// and of its ray flecks.
    pub light: [f32; 3],
    /// Ring spacing, mm.
    pub ring_mm: f32,
    /// How strongly latewood shows (0: no rings, 1: full latewood colour).
    pub contrast: f32,
    /// How far the rings wander (figure), mm.
    pub figure_mm: f32,
    /// Pore density, 0..1 (oak and ash: high; maple and birch: low).
    pub pores: f32,
    /// How much streak colour shows, 0..1.
    pub streaks: f32,
    /// Roughness of earlywood and of latewood.
    pub roughness: [f32; 2],
    /// Noise seed: two boards of one species differ by their seed.
    pub seed: u32,
}

/// A species: a stable id (the starter preset id without `starter.wood.`) and its look.
pub struct Species {
    pub id: &'static str,
    pub look: WoodLook,
}

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

#[allow(clippy::too_many_arguments)]
const fn species(
    id: &'static str,
    early: [f32; 3],
    late: [f32; 3],
    streak: [f32; 3],
    light: [f32; 3],
    ring_mm: f32,
    contrast: f32,
    figure_mm: f32,
    pores: f32,
    streaks: f32,
    roughness: [f32; 2],
) -> Species {
    Species { id, look: WoodLook { early, late, streak, light, ring_mm, contrast, figure_mm, pores, streaks, roughness, seed: 0 } }
}

/// The starter species, in the starter pack's order.
pub const SPECIES: &[Species] = &[
    species("oak-honey", rgb(204, 160, 102), rgb(150, 104, 56), rgb(132, 92, 52), rgb(226, 188, 130), 3.0, 0.7, 2.5, 0.8, 0.3, [0.60, 0.70]),
    species("oak-smoked", rgb(110, 88, 68), rgb(62, 46, 34), rgb(48, 36, 28), rgb(134, 110, 86), 3.0, 0.7, 2.5, 0.8, 0.3, [0.64, 0.74]),
    // Walnut: chocolate heartwood with lighter golden-brown zones, near-black streaks and open
    // pores (no sapwood band).
    species("walnut-dark", rgb(96, 67, 47), rgb(56, 39, 29), rgb(42, 30, 25), rgb(128, 93, 63), 4.0, 0.45, 4.0, 0.6, 0.55, [0.52, 0.64]),
    species("maple-light", rgb(232, 208, 164), rgb(206, 174, 124), rgb(190, 156, 110), rgb(244, 226, 188), 3.5, 0.3, 2.0, 0.1, 0.2, [0.52, 0.60]),
    species("cherry-warm", rgb(180, 112, 74), rgb(140, 80, 50), rgb(116, 64, 42), rgb(204, 140, 96), 3.0, 0.4, 2.5, 0.2, 0.35, [0.54, 0.62]),
    species("ash-pale", rgb(222, 200, 162), rgb(164, 130, 88), rgb(150, 120, 84), rgb(238, 220, 186), 3.5, 0.75, 2.5, 0.75, 0.2, [0.64, 0.76]),
    species("birch-cream", rgb(232, 214, 176), rgb(210, 184, 140), rgb(196, 168, 126), rgb(244, 230, 198), 3.0, 0.3, 2.0, 0.1, 0.2, [0.58, 0.66]),
    species("teak-golden", rgb(178, 132, 74), rgb(128, 90, 46), rgb(92, 66, 38), rgb(206, 164, 100), 4.5, 0.5, 3.0, 0.45, 0.5, [0.56, 0.66]),
];

/// A species by id (`walnut-dark`) or starter preset id (`starter.wood.walnut-dark`).
pub fn species_look(id: &str) -> Option<WoodLook> {
    let id = id.strip_prefix("starter.wood.").unwrap_or(id);
    SPECIES.iter().find(|s| s.id.eq_ignore_ascii_case(id)).map(|s| s.look)
}

/// A shaded wood sample: sRGB colour (0..1) and roughness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WoodSample {
    pub srgb: [f32; 3],
    pub roughness: f32,
}

/// A body's wood: its look, and the affine map from the coordinates a renderer draws it in
/// (world, mm) to grain space (rows: grain = row · (p, 1)).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WoodShade {
    pub look: WoodLook,
    pub to_grain: [[f32; 4]; 3],
}

impl WoodShade {
    /// The point `p` (world, mm) in grain space.
    pub fn grain_point(&self, p: Vec3) -> [f32; 3] {
        let [x, y, z] = [p.x as f32, p.y as f32, p.z as f32];
        self.to_grain.map(|r| r[0] * x + r[1] * y + r[2] * z + r[3])
    }

    /// The wood's colour and roughness at `p` (world, mm).
    pub fn sample(&self, p: Vec3) -> WoodSample {
        sample(&self.look, self.grain_point(p))
    }

    /// The same shade seen through a rigid or affine move `m` (rows, world ← local): for a
    /// renderer that draws the body in its local coordinates.
    pub fn through(&self, m: [[f64; 4]; 3]) -> WoodShade {
        let g = self.to_grain.map(|r| r.map(f64::from));
        let mut out = [[0f32; 4]; 3];
        for (i, row) in g.iter().enumerate() {
            for j in 0..4 {
                let mut v = (0..3).map(|k| row[k] * m[k][j]).sum::<f64>();
                if j == 3 {
                    v += row[3];
                }
                out[i][j] = v as f32;
            }
        }
        WoodShade { look: self.look, to_grain: out }
    }

    /// The shade as the GPU reads it: 8 `vec4`s (see [`WGSL`]).
    pub fn texels(&self) -> [[f32; 4]; TEXELS] {
        let l = &self.look;
        let [r0, r1, r2] = self.to_grain;
        [
            [l.early[0], l.early[1], l.early[2], l.ring_mm],
            [l.late[0], l.late[1], l.late[2], l.contrast],
            [l.streak[0], l.streak[1], l.streak[2], l.figure_mm],
            [l.pores, l.streaks, l.roughness[0], l.roughness[1]],
            // The seed as an exact f32 (below 2^24).
            [(l.seed & 0x00ff_ffff) as f32, l.light[0], l.light[1], l.light[2]],
            r0,
            r1,
            r2,
        ]
    }
}

/// `vec4`s per wood slot on the GPU.
pub const TEXELS: usize = 8;

// Integer hash → [0, 1). Kept bit for bit equal to the WGSL `wood_hash`.
fn hash(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f) ^ seed;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / 16_777_216.0
}

fn mixf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// Trilinear value noise in [0, 1), smooth (C1) between lattice points.
fn noise(p: [f32; 3], seed: u32) -> f32 {
    let i = p.map(f32::floor);
    let f = [p[0] - i[0], p[1] - i[1], p[2] - i[2]];
    let u = f.map(|v| v * v * (3.0 - 2.0 * v));
    let [x, y, z] = i.map(|v| v as i32);
    let h = |dx: i32, dy: i32, dz: i32| hash(x.wrapping_add(dx), y.wrapping_add(dy), z.wrapping_add(dz), seed);
    let x00 = mixf(h(0, 0, 0), h(1, 0, 0), u[0]);
    let x10 = mixf(h(0, 1, 0), h(1, 1, 0), u[0]);
    let x01 = mixf(h(0, 0, 1), h(1, 0, 1), u[0]);
    let x11 = mixf(h(0, 1, 1), h(1, 1, 1), u[0]);
    mixf(mixf(x00, x10, u[1]), mixf(x01, x11, u[1]), u[2])
}

fn scaled(g: [f32; 3], along: f32, across: f32) -> [f32; 3] {
    [g[0] * along, g[1] * across, g[2] * across]
}

/// The wood function, at point `g` in grain space (mm). Mirrors the WGSL `wood_sample`.
pub fn sample(l: &WoodLook, g: [f32; 3]) -> WoodSample {
    let s = l.seed;
    // Figure: the rings wander, slowly along the grain and a little faster across it.
    let w = scaled(g, 0.004, 0.02);
    let dy = noise(w, s) - 0.5;
    let dz = noise([w[0] + 17.0, w[1] + 31.0, w[2] + 47.0], s ^ 0x9e37_79b9) - 0.5;
    let y = g[1] + 2.0 * l.figure_mm * dy;
    let z = g[2] + 2.0 * l.figure_mm * dz;
    let r = (y * y + z * z).sqrt();
    // Rings: uneven widths from year to year.
    let ring = r / l.ring_mm + 0.6 * (noise(scaled([g[0], y, z], 0.01, 0.04), s ^ 0x85eb_ca6b) - 0.5);
    let t = ring - ring.floor();
    // Earlywood grades into latewood across the ring, which ends sharply at the next ring.
    let late = smoothstep(0.55, 0.95, t) * l.contrast;
    let q = [g[0], y, z];
    // Colour zones: broad bands of the lighter, warmer tone that run the length of a board.
    let zone = smoothstep(0.35, 0.8, noise(scaled(q, 0.0015, 0.05), s ^ 0x1656_67b1));
    // Dark streaks: long along the grain, a few mm across.
    let streak = smoothstep(0.6, 0.92, noise(scaled(q, 0.004, 0.25), s ^ 0xc2b2_ae35)) * l.streaks;
    // Pores: the cut vessels, short dark dashes along the grain (dots on end grain), mostly in
    // the earlywood.
    let pore = smoothstep(1.0 - 0.3 * l.pores, 1.0, noise(scaled(q, 0.08, 1.3), s ^ 0x27d4_eb2f)) * l.pores.min(1.0) * (1.0 - 0.5 * late);
    // Ray flecks: short pale ticks.
    let fleck = smoothstep(0.8, 0.97, noise(scaled(q, 0.3, 1.6), s ^ 0x6a09_e667)) * (0.25 + 0.5 * l.pores);
    // Fibres along the grain, coarse and fine.
    let fibre = (0.94 + 0.12 * noise(scaled(q, 0.03, 1.2), s ^ 0x3c6e_f372)) * (0.96 + 0.08 * noise(scaled(q, 0.05, 4.0), s ^ 0xbb67_ae85));
    let mut c = [0f32; 3];
    for k in 0..3 {
        let base = mixf(mixf(l.early[k], l.light[k], zone), l.late[k], late);
        let streaked = mixf(base, l.streak[k], streak);
        let flecked = mixf(streaked, l.light[k], 0.35 * fleck);
        c[k] = (flecked * (1.0 - 0.55 * pore) * fibre).clamp(0.0, 1.0);
    }
    let roughness = (mixf(l.roughness[0], l.roughness[1], late / l.contrast.max(1e-3)) + 0.08 * pore).clamp(0.02, 1.0);
    WoodSample { srgb: c, roughness }
}

/// The GPU twin of [`sample`]: `wood_sample(slot, g)` returns sRGB colour and roughness, reading
/// the slot's [`TEXELS`] `vec4`s through `wood_texel(slot, k)` (which the including shader
/// defines). `wood_grain(slot, p)` maps a world point into grain space.
pub const WGSL: &str = include_str!("wood.wgsl");

/// How the grain axis is chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Axis {
    /// Along the body's longest direction (its principal axis), so boards run lengthwise and
    /// legs run along their length, even when tapered or splayed.
    Long,
    /// A fixed direction (world).
    Fixed([f64; 3]),
}

/// How a body's grain frame is set up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub axis: Axis,
    /// Scale of the whole pattern (1: real size).
    pub scale: f64,
    /// Where the pith (the log's centre) lies from the body's middle, mm: `[across its width,
    /// across its thickness]`. `[0, 0]` (the default) centres the rings on the board, so they
    /// grow out from its middle; moving it out places the board elsewhere in the log, toward
    /// its bark (cathedral arches on a flat-sawn face, straight lines once far away).
    pub pith_mm: [f64; 2],
}

impl Default for Frame {
    fn default() -> Self {
        Frame { axis: Axis::Long, scale: 1.0, pith_mm: [0.0, 0.0] }
    }
}

/// The grain map (world → grain space) of a body drawn as `triangles` over `positions` (world,
/// mm): the grain runs along `frame.axis` (or the body's principal axis), the pith lies
/// `frame.pith_mm` from the body's middle (across its width, across its thickness), and grain coordinates
/// along the axis count from the body's end, so a body that grows longer keeps the grain it had.
/// `None` for a body with no area.
pub fn frame_for_mesh(positions: &[Vec3], triangles: &[[u32; 3]], frame: Frame) -> Option<[[f32; 4]; 3]> {
    // Area-weighted centroid and covariance of the surface (triangle vertices as samples).
    let mut area = 0.0;
    let mut c = Vec3::ZERO;
    for t in triangles {
        let [a, b, d] = t.map(|k| positions.get(k as usize).copied());
        let (Some(a), Some(b), Some(d)) = (a, b, d) else { continue };
        let w = (b - a).cross(d - a).len() * 0.5;
        area += w;
        c = c + (a + b + d) * (w / 3.0);
    }
    if !(area > 1e-12) {
        return None;
    }
    c = c * (1.0 / area);
    let mut m = [[0f64; 3]; 3];
    for t in triangles {
        let [a, b, d] = t.map(|k| positions.get(k as usize).copied());
        let (Some(a), Some(b), Some(d)) = (a, b, d) else { continue };
        // The triangle's exact second moment, so the frame does not depend on how the
        // surface happens to be triangulated: A/12 (Σ vᵢvᵢᵀ + s sᵀ), s = Σ vᵢ.
        let w = (b - a).cross(d - a).len() * 0.5 / 12.0;
        let q = [a, b, d].map(|v| [v.x - c.x, v.y - c.y, v.z - c.z]);
        let s = [q[0][0] + q[1][0] + q[2][0], q[0][1] + q[1][1] + q[2][1], q[0][2] + q[1][2] + q[2][2]];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += w * (q[0][i] * q[0][j] + q[1][i] * q[1][j] + q[2][i] * q[2][j] + s[i] * s[j]);
            }
        }
    }
    let (along, thin) = principal(m);
    let along = match frame.axis {
        Axis::Long => along,
        Axis::Fixed(v) => Vec3::new(v[0], v[1], v[2]).normalized().unwrap_or(along),
    };
    // A steady sign: the principal axis' sign is arbitrary, the grain's is not.
    let along = if along.x + along.y * 1e-3 + along.z * 1e-6 < 0.0 { -along } else { along };
    // The thinnest direction, made perpendicular to the grain.
    let mut thin = thin - along * thin.dot(along);
    if thin.len() < 1e-6 {
        thin = if along.x.abs() < 0.9 { Vec3::X.cross(along) } else { Vec3::Y.cross(along) };
    }
    let thin = thin.normalized().unwrap_or(Vec3::Z);
    let thin = if thin.x * 1e-6 + thin.y * 1e-3 + thin.z < 0.0 { -thin } else { thin };
    let wide = along.cross(thin);
    // From the body's start along the grain, so lengthening the far end keeps the pattern.
    let start = positions.iter().map(|p| p.dot(along)).fold(f64::INFINITY, f64::min);
    let pith = c + wide * frame.pith_mm[0] + thin * frame.pith_mm[1];
    let s = 1.0 / frame.scale.clamp(0.01, 100.0);
    let row = |axis: Vec3, offset: f64| [(axis.x * s) as f32, (axis.y * s) as f32, (axis.z * s) as f32, (-offset * s) as f32];
    Some([row(along, start), row(wide, pith.dot(wide)), row(thin, pith.dot(thin))])
}

// The largest and the smallest eigenvector of a symmetric 3×3 matrix (power iteration, from
// fixed starts, so the same input always gives the same frame).
fn principal(m: [[f64; 3]; 3]) -> (Vec3, Vec3) {
    let apply = |m: &[[f64; 3]; 3], v: Vec3| Vec3::new(m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z, m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z, m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z);
    let power = |m: &[[f64; 3]; 3]| {
        // Start from the matrix's largest column, so the start is never orthogonal to the answer.
        let cols = [Vec3::new(m[0][0], m[1][0], m[2][0]), Vec3::new(m[0][1], m[1][1], m[2][1]), Vec3::new(m[0][2], m[1][2], m[2][2])];
        let mut v = cols.iter().copied().fold(Vec3::new(1.0, 0.7, 0.3), |best, c| if c.len() > best.len() { c } else { best });
        v = v.normalized().unwrap_or(Vec3::X);
        for _ in 0..64 {
            let n = apply(m, v);
            match n.normalized() {
                Some(n) => v = n,
                None => break,
            }
        }
        v
    };
    let big = power(&m);
    // The smallest: the largest of (trace·I − m), which has the same eigenvectors, reversed.
    let tr = m[0][0] + m[1][1] + m[2][2];
    let mut flipped = m.map(|r| r.map(|x| -x));
    for (i, row) in flipped.iter_mut().enumerate() {
        row[i] += tr;
    }
    let small = power(&flipped);
    (big, small)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> (Vec<Vec3>, Vec<[u32; 3]>) {
        // A 600 × 100 × 20 box along X.
        let (x, y, z) = (600.0, 100.0, 20.0);
        let p: Vec<Vec3> = (0..8).map(|i| Vec3::new(if i & 1 == 0 { 0.0 } else { x }, if i & 2 == 0 { 0.0 } else { y }, if i & 4 == 0 { 0.0 } else { z })).collect();
        let t = vec![[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
        (p, t)
    }

    #[test]
    fn a_board_runs_lengthwise_with_the_pith_in_its_middle_unless_moved() {
        let (p, t) = board();
        let m = frame_for_mesh(&p, &t, Frame::default()).unwrap();
        // Grain along X.
        assert!((m[0][0].abs() - 1.0).abs() < 1e-4, "{m:?}");
        // Thinnest direction Z: row 2 is ±Z.
        assert!((m[2][2].abs() - 1.0).abs() < 1e-4, "{m:?}");
        // The board's start is grain X 0.
        let w = WoodShade { look: species_look("walnut-dark").unwrap(), to_grain: m };
        let g = w.grain_point(Vec3::new(0.0, 50.0, 10.0));
        assert!(g[0].abs() < 1e-3, "{g:?}");
        assert!(g[1].abs() < 1e-3 && g[2].abs() < 1e-3, "{g:?}");
        // Moved 140 mm across the thickness, the board's middle is 140 mm from the pith.
        let m = frame_for_mesh(&p, &t, Frame { pith_mm: [0.0, 140.0], ..Frame::default() }).unwrap();
        let g = WoodShade { look: w.look, to_grain: m }.grain_point(Vec3::new(0.0, 50.0, 10.0));
        assert!(g[1].abs() < 1e-3 && (g[2].abs() - 140.0).abs() < 1e-2, "{g:?}");
    }

    #[test]
    fn a_longer_board_keeps_its_grain() {
        let (p, t) = board();
        let longer: Vec<Vec3> = p.iter().map(|v| if v.x > 1.0 { Vec3::new(800.0, v.y, v.z) } else { *v }).collect();
        let a = WoodShade { look: species_look("walnut-dark").unwrap(), to_grain: frame_for_mesh(&p, &t, Frame::default()).unwrap() };
        let b = WoodShade { look: a.look, to_grain: frame_for_mesh(&longer, &t, Frame::default()).unwrap() };
        for q in [Vec3::new(10.0, 3.0, 20.0), Vec3::new(300.0, 70.0, 20.0), Vec3::new(590.0, 99.0, 0.0)] {
            assert_eq!(a.sample(q).srgb.map(|v| (v * 255.0).round()), b.sample(q).srgb.map(|v| (v * 255.0).round()), "{q:?}");
        }
    }

    #[test]
    fn a_splayed_leg_runs_along_its_own_axis() {
        // A long thin box turned 7 degrees about X.
        let (p, t) = board();
        let (s, c) = 7f64.to_radians().sin_cos();
        // Long axis X → rotated toward Z.
        let turned: Vec<Vec3> = p.iter().map(|v| Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)).collect();
        let m = frame_for_mesh(&turned, &t, Frame::default()).unwrap();
        let axis = Vec3::new(m[0][0] as f64, m[0][1] as f64, m[0][2] as f64);
        assert!(axis.dot(Vec3::new(c, 0.0, s)).abs() > 0.9999, "{axis:?}");
    }

    #[test]
    fn samples_are_in_range_and_deterministic() {
        for sp in SPECIES {
            let mut l = sp.look;
            l.seed = 1234;
            for i in 0..500 {
                let g = [i as f32 * 3.7 - 900.0, (i * 7 % 113) as f32 - 50.0, 140.0 + (i % 19) as f32];
                let a = sample(&l, g);
                assert_eq!(a, sample(&l, g));
                assert!(a.srgb.iter().all(|v| (0.0..=1.0).contains(v)) && (0.02..=1.0).contains(&a.roughness), "{} {a:?}", sp.id);
            }
        }
    }

    #[test]
    fn every_starter_wood_has_a_species() {
        for id in ["oak-honey", "oak-smoked", "walnut-dark", "maple-light", "cherry-warm", "ash-pale", "birch-cream", "teak-golden"] {
            assert!(species_look(&format!("starter.wood.{id}")).is_some(), "{id}");
        }
    }
}
