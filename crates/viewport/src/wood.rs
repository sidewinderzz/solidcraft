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
    /// Glued-up stock, in grain mm: the boards' width and length (0: one piece; length 0: full
    /// length), then where the pith lies from the body's middle across its width (the boards
    /// are laid out from the body's middle, not from the pith).
    pub stock: [f32; 3],
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
    Species { id, look: WoodLook { early, late, streak, light, ring_mm, contrast, figure_mm, pores, streaks, roughness, seed: 0, stock: [0.0; 3] } }
}

/// A species as raw, unfinished timber: greyer, paler, flatter and matt (what an oil or a
/// lacquer then deepens into the finished look).
const fn raw(id: &'static str, finished: &Species) -> Species {
    const fn dull(c: [f32; 3]) -> [f32; 3] {
        const fn f(v: f32, grey: f32) -> f32 {
            (v + (grey - v) * 0.25) * 0.78 + 0.2
        }
        let grey = 0.3 * c[0] + 0.55 * c[1] + 0.15 * c[2];
        [f(c[0], grey), f(c[1], grey), f(c[2], grey)]
    }
    let l = finished.look;
    Species {
        id,
        look: WoodLook { early: dull(l.early), late: dull(l.late), streak: dull(l.streak), light: dull(l.light), contrast: l.contrast * 0.8, streaks: l.streaks * 0.8, roughness: [0.80, 0.88], ..l },
    }
}

const OAK_HONEY: Species = species("oak-honey", rgb(196, 148, 88), rgb(144, 96, 50), rgb(124, 84, 46), rgb(222, 180, 118), 3.0, 0.7, 2.5, 0.8, 0.3, [0.34, 0.44]);
const OAK_SMOKED: Species = species("oak-smoked", rgb(106, 84, 64), rgb(60, 44, 32), rgb(46, 34, 26), rgb(132, 106, 82), 3.0, 0.7, 2.5, 0.8, 0.3, [0.36, 0.46]);
// Walnut with a clear finish: warm chocolate heartwood, amber zones, near-black streaks, open
// pores (no sapwood band).
const WALNUT: Species = species("walnut-dark", rgb(112, 70, 44), rgb(70, 43, 29), rgb(56, 36, 27), rgb(142, 92, 56), 4.0, 0.55, 4.5, 0.6, 0.45, [0.30, 0.38]);
// Raw walnut is its own colours: a cool grey-brown with a faint purple cast.
const WALNUT_RAW: Species = species("walnut-dark-raw", rgb(146, 121, 104), rgb(112, 90, 77), rgb(92, 74, 68), rgb(170, 146, 122), 4.0, 0.4, 4.5, 0.6, 0.35, [0.80, 0.88]);
const MAPLE: Species = species("maple-light", rgb(228, 200, 150), rgb(202, 166, 112), rgb(186, 148, 100), rgb(242, 220, 176), 3.5, 0.3, 2.0, 0.1, 0.2, [0.30, 0.38]);
const CHERRY: Species = species("cherry-warm", rgb(176, 104, 66), rgb(136, 74, 44), rgb(110, 58, 36), rgb(202, 134, 88), 3.0, 0.4, 2.5, 0.2, 0.35, [0.30, 0.38]);
const ASH: Species = species("ash-pale", rgb(218, 194, 152), rgb(160, 124, 80), rgb(146, 114, 76), rgb(236, 216, 178), 3.5, 0.75, 2.5, 0.75, 0.2, [0.36, 0.46]);
const BIRCH: Species = species("birch-cream", rgb(230, 208, 164), rgb(208, 178, 130), rgb(192, 162, 116), rgb(244, 228, 190), 3.0, 0.3, 2.0, 0.1, 0.2, [0.32, 0.40]);
const TEAK: Species = species("teak-golden", rgb(174, 124, 64), rgb(124, 84, 40), rgb(88, 60, 32), rgb(204, 158, 90), 4.5, 0.5, 3.0, 0.45, 0.5, [0.34, 0.42]);

/// The starter species, in the starter pack's order: each with a clear finish (`oak-honey`) and
/// as raw timber (`oak-honey-raw`).
pub const SPECIES: &[Species] = &[
    OAK_HONEY,
    raw("oak-honey-raw", &OAK_HONEY),
    OAK_SMOKED,
    raw("oak-smoked-raw", &OAK_SMOKED),
    WALNUT,
    WALNUT_RAW,
    MAPLE,
    raw("maple-light-raw", &MAPLE),
    CHERRY,
    raw("cherry-warm-raw", &CHERRY),
    ASH,
    raw("ash-pale-raw", &ASH),
    BIRCH,
    raw("birch-cream-raw", &BIRCH),
    TEAK,
    raw("teak-golden-raw", &TEAK),
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
        sample(&self.look, self.grain_point(p), [0.0; 3], [0.0; 3])
    }

    /// A world step (mm) in grain space.
    pub fn grain_step(&self, v: Vec3) -> [f32; 3] {
        let [x, y, z] = [v.x as f32, v.y as f32, v.z as f32];
        self.to_grain.map(|r| r[0] * x + r[1] * y + r[2] * z)
    }

    /// The same, as a pixel sees it whose steps across the screen are `dx` and `dy` on the
    /// surface (world, mm): detail finer than the pixel fades to its average, so a far or a
    /// steep view is calm and does not shimmer.
    pub fn sample_seen(&self, p: Vec3, dx: Vec3, dy: Vec3) -> WoodSample {
        sample(&self.look, self.grain_point(p), self.grain_step(dx), self.grain_step(dy))
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

    /// The shade as the GPU reads it: 9 `vec4`s (see [`WGSL`]).
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
            [l.stock[0], l.stock[1], l.stock[2], 0.0],
        ]
    }
}

/// `vec4`s per wood slot on the GPU.
pub const TEXELS: usize = 9;

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

/// The wood function, at point `g` in grain space (mm), seen by a pixel whose steps across the
/// screen are `fx` and `fy` in grain space (zero: a point). Detail finer than the pixel fades
/// to its average, each kind by the footprint in its own direction: rings by how many of them
/// a pixel crosses, fibres and pores by the pixel's width across the grain. So a far or a
/// steep view is calm, and a steep view along the grain keeps its lines. Mirrors the WGSL
/// `wood_sample`.
pub fn sample(l: &WoodLook, g: [f32; 3], fx: [f32; 3], fy: [f32; 3]) -> WoodSample {
    let s = l.seed;
    // The pixel's width across the grain and its length along it, mm.
    let across = (fx[1] * fx[1] + fx[2] * fx[2]).sqrt().max((fy[1] * fy[1] + fy[2] * fy[2]).sqrt());
    let along = fx[0].abs().max(fy[0].abs());
    // Glued-up stock: the body is made of boards `stock[0]` wide (and `stock[1]` long, 0: full
    // length), each cut from its own place in a log, with a glue line between them.
    let mut g = g;
    let mut seam = 0.0;
    let mut tone = 1.0;
    if l.stock[0] > 0.0 {
        let wide = l.stock[0];
        let yb = g[1] + l.stock[2];
        let j = (yb / wide + 0.5).floor();
        let ji = j as i32;
        let mut ii = 0;
        let mut butt = 1.0e9;
        if l.stock[1] > 0.0 {
            let xs = g[0] + hash(ji, 0, 0, s ^ 0x2545_f491) * l.stock[1];
            let i = (xs / l.stock[1]).floor();
            let u = xs - i * l.stock[1];
            butt = u.min(l.stock[1] - u);
            ii = i as i32;
        }
        let h1 = hash(ji, ii, 1, s ^ 0x2545_f491);
        let h2 = hash(ji, ii, 2, s ^ 0x2545_f491);
        let h3 = hash(ji, ii, 3, s ^ 0x2545_f491);
        let h4 = hash(ji, ii, 4, s ^ 0x2545_f491);
        let yl = yb - j * wide;
        let edge = (0.5 * wide - yl.abs()).min(butt);
        let line_mm = 0.15 + across.max(along);
        seam = 0.45 * (1.0 - smoothstep(0.0, line_mm, edge)) * (0.15 / line_mm);
        tone = 0.93 + 0.14 * h4;
        let side = if h3 < 0.5 { -1.0 } else { 1.0 };
        g = [g[0] + 40000.0 * h3, yl - l.stock[2] - (h1 - 0.5) * wide * 1.2, g[2] - side * (15.0 + 120.0 * h2)];
    }
    // Figure: the rings wander, slowly along the grain and a little faster across it.
    let w = scaled(g, 0.004, 0.02);
    let dy = noise(w, s) - 0.5;
    let dz = noise([w[0] + 17.0, w[1] + 31.0, w[2] + 47.0], s ^ 0x9e37_79b9) - 0.5;
    // And they sweep in long, slow waves: the flowing cathedral figure of a flat-sawn board.
    let w2 = scaled(g, 0.0012, 0.006);
    let ey = noise([w2[0] + 5.0, w2[1] + 11.0, w2[2] + 23.0], s ^ 0x510e_527f) - 0.5;
    let ez = noise([w2[0] + 41.0, w2[1] + 3.0, w2[2] + 29.0], s ^ 0x9b05_688c) - 0.5;
    let y = g[1] + l.figure_mm * (2.0 * dy + 9.0 * ey);
    let z = g[2] + l.figure_mm * (2.0 * dz + 9.0 * ez);
    let r = (y * y + z * z).sqrt();
    // Rings a pixel crosses: its steps, taken outward from the pith.
    let fr = ((y * fx[1] + z * fx[2]).abs() + (y * fy[1] + z * fy[2]).abs()) / (r.max(1.0e-3) * l.ring_mm);
    let rings = 1.0 - smoothstep(0.35, 1.0, fr);
    let thin = 1.0 - smoothstep(0.08, 0.35, fr);
    // Rings: uneven widths from year to year.
    let ring = r / l.ring_mm + 0.6 * (noise(scaled([g[0], y, z], 0.01, 0.04), s ^ 0x85eb_ca6b) - 0.5);
    let t = ring - ring.floor();
    // Earlywood grades into latewood across the ring, which ends at the next ring: sharply
    // close up, over a pixel's width from afar (a hard edge finer than a pixel would shimmer).
    let wrap = 1.0 - smoothstep(1.0 - fr.clamp(0.02, 0.5), 1.0, t);
    let late = mixf(0.25, smoothstep(0.55, 0.95, t) * wrap, rings) * l.contrast;
    let q = [g[0], y, z];
    // Colour zones: broad bands of the lighter, warmer tone that run the length of a board.
    // The same point by its ring: along the grain, out from the pith, and around it. Colour
    // belongs to the rings, so its zones and streaks bend with them (the cathedral arches).
    let around = 20.0 * y / (r + 1.0);
    let zone = smoothstep(0.35, 0.8, noise([g[0] * 0.0015, r * 0.05, around * 0.05], s ^ 0x1656_67b1));
    // Dark streaks: long along the grain, a few mm across.
    let streak = smoothstep(0.6, 0.92, noise([g[0] * 0.004, r * 0.25, around * 0.03], s ^ 0xc2b2_ae35)) * l.streaks;
    // How much of each size of detail a pixel this wide still resolves.
    let fine = 1.0 - smoothstep(0.08, 0.25, across);
    let coarse = 1.0 - smoothstep(0.25, 0.8, across);
    let mid = 1.0 - smoothstep(0.8, 2.5, across);
    // Pores: the cut vessels, short dark dashes along the grain (dots on end grain), mostly in
    // the earlywood.
    let pore = smoothstep(1.0 - 0.3 * l.pores, 1.0, noise(scaled(q, 0.1, 1.8), s ^ 0x27d4_eb2f)) * l.pores.min(1.0) * (1.0 - 0.5 * late) * coarse * (1.0 - smoothstep(3.0, 10.0, along));
    // Ray flecks: short pale ticks.
    let fleck = smoothstep(0.8, 0.97, noise(scaled(q, 0.3, 1.6), s ^ 0x6a09_e667)) * (0.25 + 0.5 * l.pores) * coarse * (1.0 - smoothstep(1.0, 3.3, along));
    // Fibres along the grain: coarse, fine, and broad soft ones that still show from afar.
    let fibre = mixf(1.0, 0.92 + 0.16 * noise(scaled(q, 0.03, 1.2), s ^ 0x3c6e_f372), coarse)
        * mixf(1.0, 0.94 + 0.12 * noise(scaled(q, 0.05, 4.0), s ^ 0xbb67_ae85), fine)
        * mixf(1.0, 0.95 + 0.1 * noise(scaled(q, 0.012, 0.45), s ^ 0xa54f_f53a), mid);
    // The crisp dark line that ends each year's growth.
    let line = smoothstep(0.88, 0.97, t) * wrap * thin * l.contrast;
    // Ripple: faint bands across the grain, where the fibres dip and rise.
    let ripple = mixf(1.0, 0.95 + 0.1 * noise(scaled(g, 0.06, 0.012), s ^ 0x1f83_d9ab), 1.0 - smoothstep(5.0, 16.0, along));
    let mut c = [0f32; 3];
    for k in 0..3 {
        let base = mixf(mixf(l.early[k], l.light[k], zone), l.late[k], late);
        let streaked = mixf(base, l.streak[k], streak);
        let flecked = mixf(streaked, l.light[k], 0.35 * fleck);
        c[k] = (flecked * (1.0 - 0.7 * pore) * (1.0 - 0.2 * line) * fibre * ripple * tone * (1.0 - seam)).clamp(0.0, 1.0);
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
    /// How far the grain is slid along its length, mm.
    pub shift_mm: f64,
}

impl Default for Frame {
    fn default() -> Self {
        Frame { axis: Axis::Long, scale: 1.0, pith_mm: [0.0, 0.0], shift_mm: 0.0 }
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
    Some([row(along, start + frame.shift_mm), row(wide, pith.dot(wide)), row(thin, pith.dot(thin))])
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
                if i % 2 == 0 {
                    l.stock = [80.0, (i % 3) as f32 * 400.0, 12.0];
                }
                let (fx, fy) = ([(i % 4) as f32, 0.2, 0.0], [0.0, 0.1, (i % 3) as f32 * 0.4]);
                let a = sample(&l, g, fx, fy);
                assert_eq!(a, sample(&l, g, fx, fy));
                assert!(a.srgb.iter().all(|v| (0.0..=1.0).contains(v)) && (0.02..=1.0).contains(&a.roughness), "{} {a:?}", sp.id);
            }
        }
    }

    /// A steep view along the grain (a pixel long along it, narrow across it) keeps its rings
    /// and fibres; a pixel wide across the grain fades them to their average.
    #[test]
    fn detail_fades_by_the_pixels_width_across_the_grain_only() {
        let mut l = species_look("walnut-dark").unwrap();
        l.seed = 99;
        let spread = |fx: [f32; 3], fy: [f32; 3]| {
            let v: Vec<f32> = (0..400).map(|i| sample(&l, [300.0, 30.0 + i as f32 * 0.137, 60.0], fx, fy).srgb[0]).collect();
            let mean = v.iter().sum::<f32>() / v.len() as f32;
            (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / v.len() as f32).sqrt()
        };
        let sharp = spread([0.0; 3], [0.0; 3]);
        let steep = spread([6.0, 0.0, 0.0], [0.0, 0.05, 0.0]);
        let far = spread([0.0, 6.0, 0.0], [0.0, 0.0, 6.0]);
        assert!(steep > 0.6 * sharp, "along the grain: {steep} of {sharp}");
        assert!(far < 0.5 * sharp, "across the grain: {far} of {sharp}");
    }

    /// Glued-up stock: each board has its own grain, with a darker glue line between two.
    #[test]
    fn glued_up_boards_differ_and_show_their_glue_lines() {
        let mut l = species_look("maple-light").unwrap();
        l.seed = 5;
        let plain = l;
        l.stock = [100.0, 0.0, 0.0];
        let at = |l: &WoodLook, y: f32| sample(l, [250.0, y, 8.0], [0.0; 3], [0.0; 3]).srgb;
        // Boards are laid out from the body's middle: one spans -50…50, the next 50…150.
        let (a, b): (Vec<_>, Vec<_>) = (0..40).map(|i| (at(&l, -40.0 + i as f32 * 2.0), at(&l, 60.0 + i as f32 * 2.0))).unzip();
        assert_ne!(a, b, "two boards, two grains");
        let lum = |c: [f32; 3]| c[0] + c[1] + c[2];
        let seam = lum(at(&l, 50.0));
        let beside = (lum(at(&l, 48.0)) + lum(at(&l, 52.0))) / 2.0;
        assert!(seam < 0.8 * beside, "the glue line is darker: {seam} beside {beside}");
        assert_ne!(at(&l, 10.0), at(&plain, 10.0));
    }

    /// Sliding the grain along its length moves the same pattern.
    #[test]
    fn a_shift_slides_the_grain_along_the_board() {
        let (p, t) = board();
        let look = species_look("oak-honey").unwrap();
        let a = WoodShade { look, to_grain: frame_for_mesh(&p, &t, Frame::default()).unwrap() };
        let b = WoodShade { look, to_grain: frame_for_mesh(&p, &t, Frame { shift_mm: 40.0, pith_mm: [5.0, -3.0], ..Frame::default() }).unwrap() };
        let along = Vec3::new(f64::from(a.to_grain[0][0]), f64::from(a.to_grain[0][1]), f64::from(a.to_grain[0][2]));
        let wide = Vec3::new(f64::from(a.to_grain[1][0]), f64::from(a.to_grain[1][1]), f64::from(a.to_grain[1][2]));
        let thin = Vec3::new(f64::from(a.to_grain[2][0]), f64::from(a.to_grain[2][1]), f64::from(a.to_grain[2][2]));
        for i in 0..50 {
            let q = Vec3::new(10.0 + f64::from(i) * 7.0, 20.0 + f64::from(i % 7) * 5.0, 3.0);
            let (x, y) = (a.grain_point(q), b.grain_point(q + along * 40.0 + wide * 5.0 - thin * 3.0));
            assert!((0..3).all(|k| (x[k] - y[k]).abs() < 1e-3), "{x:?} {y:?}");
        }
    }

    #[test]
    fn every_starter_wood_has_a_species() {
        for id in ["oak-honey", "oak-smoked", "walnut-dark", "maple-light", "cherry-warm", "ash-pale", "birch-cream", "teak-golden"] {
            let finished = species_look(&format!("starter.wood.{id}")).unwrap();
            let raw = species_look(&format!("starter.wood.{id}-raw")).unwrap();
            // Raw timber is matt; a finish gives it a sheen.
            assert!(raw.roughness[0] > finished.roughness[0] + 0.3, "{id}");
        }
    }
}
