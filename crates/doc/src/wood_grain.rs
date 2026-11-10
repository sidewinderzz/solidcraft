//! Procedural wood grain on a look: which species, and how its grain runs through each body
//! (`solidcraft_viewport::wood`). Saved with the look; a look without it is a plain colour, as
//! before.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use solidcraft_viewport::wood::{self, Axis, Frame, WoodShade};

/// A look's wood grain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "GrainWire", into = "GrainWire")]
pub struct Grain {
    /// The species (`walnut-dark`, or a starter id `starter.wood.walnut-dark`).
    pub species: String,
    pub axis: GrainAxis,
    /// The noise seed. `None`: each body gets its own, from its name, so the boards of a design
    /// differ and each keeps its grain.
    pub seed: Option<u32>,
    /// Scale of the whole pattern (1: real size).
    pub scale: f64,
    /// Ring spacing, mm (`None`: the species').
    pub ring_spacing_mm: Option<f64>,
    /// How much the rings wander, relative to the species (1: as the species).
    pub figure: f64,
}

/// Which way the grain runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GrainAxis {
    /// Along each body's longest direction.
    Long,
    /// A fixed direction (unit vector, design coordinates).
    Fixed([f64; 3]),
}

impl Grain {
    /// The species' default grain.
    pub fn species(species: &str) -> Option<Grain> {
        wood::species_look(species)?;
        Some(Grain { species: species.strip_prefix("starter.wood.").unwrap_or(species).to_string(), axis: GrainAxis::Long, seed: None, scale: 1.0, ring_spacing_mm: None, figure: 1.0 })
    }

    /// Check the values (finite, in range, a known species).
    pub fn validate(&self) -> Result<(), String> {
        if wood::species_look(&self.species).is_none() {
            let ids: Vec<&str> = wood::SPECIES.iter().map(|s| s.id).collect();
            return Err(format!("unknown wood species `{}` (one of {})", self.species, ids.join(", ")));
        }
        if !(self.scale.is_finite() && (0.05..=20.0).contains(&self.scale)) {
            return Err("`grain.scale_mm`/`scale` must be 0.05…20".into());
        }
        if let Some(r) = self.ring_spacing_mm
            && !(r.is_finite() && (0.2..=50.0).contains(&r))
        {
            return Err("`grain.ring_spacing_mm` must be 0.2…50 mm".into());
        }
        if !(self.figure.is_finite() && (0.0..=5.0).contains(&self.figure)) {
            return Err("`grain.figure` must be 0…5".into());
        }
        if let GrainAxis::Fixed(v) = self.axis {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if !(l.is_finite() && l > 1e-9) {
                return Err("`grain.axis` must be a nonzero finite direction".into());
            }
        }
        Ok(())
    }

    /// The shade of a body named `body` whose display mesh is `positions`/`triangles` (world).
    pub fn shade(&self, body: &str, positions: &[solidcore_record::Vec3], triangles: &[[u32; 3]]) -> Option<WoodShade> {
        let mut look = wood::species_look(&self.species)?;
        look.seed = self.seed.unwrap_or_else(|| name_seed(body)) & 0x00ff_ffff;
        if let Some(r) = self.ring_spacing_mm {
            look.ring_mm = r as f32;
        }
        look.figure_mm *= self.figure as f32;
        let axis = match self.axis {
            GrainAxis::Long => Axis::Long,
            GrainAxis::Fixed(v) => Axis::Fixed(v),
        };
        let to_grain = wood::frame_for_mesh(positions, triangles, Frame { axis, scale: self.scale, ..Frame::default() })?;
        Some(WoodShade { look, to_grain })
    }

    /// Apply the fields of an `appearance.assign` `grain` object (or `true`: the defaults).
    pub fn apply_json(&mut self, v: &Value) -> Result<(), String> {
        let Some(o) = v.as_object() else {
            return if v.as_bool() == Some(true) { Ok(()) } else { Err("`grain` must be an object, true or null".into()) };
        };
        for (k, x) in o {
            match k.as_str() {
                "species" => self.species = x.as_str().ok_or("`grain.species` must be a string")?.trim_start_matches("starter.wood.").to_string(),
                "axis" => self.axis = parse_axis(x)?,
                "seed" => self.seed = Some(x.as_u64().filter(|s| *s < 1 << 24).ok_or("`grain.seed` must be an integer 0…16777215")? as u32),
                "scale" => self.scale = x.as_f64().ok_or("`grain.scale` must be a number")?,
                "ring_spacing_mm" => self.ring_spacing_mm = Some(x.as_f64().ok_or("`grain.ring_spacing_mm` must be a number")?),
                "figure" => self.figure = x.as_f64().ok_or("`grain.figure` must be a number")?,
                other => return Err(format!("unknown `grain` field `{other}` (species, axis, seed, scale, ring_spacing_mm, figure)")),
            }
        }
        self.validate()
    }
}

/// A body's own seed: FNV-1a of its name, 24 bits.
pub fn name_seed(name: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in name.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h & 0x00ff_ffff
}

fn parse_axis(v: &Value) -> Result<GrainAxis, String> {
    let bad = || "`grain.axis` must be \"long\", \"X\", \"Y\", \"Z\" or [x, y, z]".to_string();
    match v {
        Value::String(s) => match s.to_ascii_lowercase().as_str() {
            "long" | "auto" => Ok(GrainAxis::Long),
            "x" => Ok(GrainAxis::Fixed([1.0, 0.0, 0.0])),
            "y" => Ok(GrainAxis::Fixed([0.0, 1.0, 0.0])),
            "z" => Ok(GrainAxis::Fixed([0.0, 0.0, 1.0])),
            _ => Err(bad()),
        },
        Value::Array(a) if a.len() == 3 => {
            let c: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
            let [x, y, z] = c[..] else { return Err(bad()) };
            let l = (x * x + y * y + z * z).sqrt();
            if !(l.is_finite() && l > 1e-9) {
                return Err(bad());
            }
            Ok(GrainAxis::Fixed([x / l, y / l, z / l]))
        }
        _ => Err(bad()),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrainWire {
    species: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    axis: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ring_spacing_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    figure: Option<f64>,
}

impl TryFrom<GrainWire> for Grain {
    type Error = String;
    fn try_from(w: GrainWire) -> Result<Self, Self::Error> {
        let g = Grain {
            species: w.species,
            axis: w.axis.as_ref().map(parse_axis).transpose()?.unwrap_or(GrainAxis::Long),
            seed: w.seed.map(|s| s & 0x00ff_ffff),
            scale: w.scale.unwrap_or(1.0),
            ring_spacing_mm: w.ring_spacing_mm,
            figure: w.figure.unwrap_or(1.0),
        };
        g.validate()?;
        Ok(g)
    }
}

impl From<Grain> for GrainWire {
    fn from(g: Grain) -> Self {
        GrainWire {
            species: g.species,
            axis: match g.axis {
                GrainAxis::Long => None,
                GrainAxis::Fixed(v) => Some(Value::from(v.to_vec())),
            },
            seed: g.seed,
            scale: (g.scale != 1.0).then_some(g.scale),
            ring_spacing_mm: g.ring_spacing_mm,
            figure: (g.figure != 1.0).then_some(g.figure),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn grain_settings_round_trip_and_refuse_bad_values() {
        let mut g = Grain::species("starter.wood.walnut-dark").unwrap();
        assert_eq!(serde_json::to_value(&g).unwrap(), json!({"species": "walnut-dark"}));
        g.apply_json(&json!({"axis": "X", "seed": 7, "scale": 1.5, "ring_spacing_mm": 5.0, "figure": 0.5})).unwrap();
        let saved = serde_json::to_value(&g).unwrap();
        assert_eq!(serde_json::from_value::<Grain>(saved.clone()).unwrap(), g);
        assert_eq!(saved["axis"], json!([1.0, 0.0, 0.0]));
        for bad in [json!({"species": "balsa"}), json!({"axis": [0, 0, 0]}), json!({"scale": 0}), json!({"ring_spacing_mm": -1}), json!({"knots": true})] {
            assert!(g.clone().apply_json(&bad).is_err(), "{bad}");
        }
        assert!(serde_json::from_value::<Grain>(json!({"species": "walnut-dark", "scale": 1e9})).is_err());
    }
}
