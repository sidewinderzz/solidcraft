//! Renderer-independent product geometry. No modelling overlays or device resources.

use std::sync::Arc;

use solidcraft_geom::{Aabb3, Mesh};

#[derive(Clone, Debug, PartialEq)]
pub struct FaceAppearance {
    /// The existing mesh's B-rep face index, resolved by the document's appearance policy.
    pub face: u32,
    pub color: [u8; 3],
    pub opacity: f64,
    pub surface: Option<solidcraft_viewport::surface::OpaqueSurface>,
    pub transmission: Option<crate::transmission::Transmission>,
    /// Procedural wood grain over the face's colour and roughness (boxed: a face list can be
    /// long, and admission counts each entry's size).
    pub wood: Option<Box<solidcraft_viewport::wood::WoodShade>>,
}

#[derive(Clone, Debug)]
pub struct ProductMesh {
    /// Existing computed body name, including occurrence identity where applicable.
    pub name: String,
    /// Established world-space mesh. Adapters must not apply occurrence transforms again.
    pub mesh: Arc<Mesh>,
    pub color: [u8; 3],
    pub opacity: f64,
    pub surface: Option<solidcraft_viewport::surface::OpaqueSurface>,
    pub transmission: Option<crate::transmission::Transmission>,
    pub faces: Vec<FaceAppearance>,
    /// Procedural wood grain over the body's colour and roughness (faces without looks of
    /// their own).
    pub wood: Option<solidcraft_viewport::wood::WoodShade>,
}

impl ProductMesh {
    /// The wood on mesh face `face`: a face look's own (none for a plain face look), else the
    /// body's.
    pub fn wood_at(&self, face: Option<u32>) -> Option<&solidcraft_viewport::wood::WoodShade> {
        match face.and_then(|f| self.faces.iter().find(|a| a.face == f)) {
            Some(look) => look.wood.as_deref(),
            None => self.wood.as_ref(),
        }
    }
}

/// `surface` with the wood's colour and roughness at `point` (world, mm).
pub fn wooded_surface(
    wood: &solidcraft_viewport::wood::WoodShade,
    surface: solidcraft_viewport::surface::OpaqueSurface,
    point: solidcore_record::Vec3,
) -> solidcraft_viewport::surface::OpaqueSurface {
    let s = wood.sample(point);
    let [r, g, b] = s.srgb.map(|v| solidcraft_viewport::surface::srgb8_to_linear((v.clamp(0.0, 1.0) * 255.0).round() as u8));
    surface.with_authored_finish(solidcore_record::Vec3::new(r, g, b), surface.metallic(), f64::from(s.roughness)).unwrap_or(surface)
}

#[derive(Clone, Debug)]
pub struct ProductScene {
    pub meshes: Vec<ProductMesh>,
    pub bounds: Aabb3,
}

impl ProductScene {
    pub fn has_transmission(&self) -> bool {
        self.meshes.iter().any(|m| m.transmission.is_some() || m.faces.iter().any(|f| f.transmission.is_some()))
    }
    pub fn required_features(&self) -> Vec<crate::RenderFeature> {
        if self.meshes.iter().any(|m| m.surface.is_some() || m.faces.iter().any(|f| f.surface.is_some())) {
            vec![crate::RenderFeature::Pbr]
        } else {
            Vec::new()
        }
    }
}
