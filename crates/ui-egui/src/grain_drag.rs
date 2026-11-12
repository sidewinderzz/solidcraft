//! Dragging a wood's grain through its body: with the Appearance dialog's Move grain on, a
//! left-button drag on a wooden body slides the grain with the pointer, in the plane of the
//! grain's own directions that faces the view most squarely. Across the grain this moves the log's centre (the
//! pith) away from the board, so the board is cut from another part of the log; along the grain
//! it slides the figure along the board. Each frame assigns the dialog's look with the new
//! position; the whole drag is one undo step.

use std::sync::Arc;

use egui::Pos2;
use serde_json::{Value, json};
use solidcore_record::Vec3;
use solidcraft_engine::doc::Document;
use solidcraft_engine::doc::wood_grain::MAX_PITH_MM;
use solidcraft_engine::geom::prelude::*;

use crate::SolveApp;
use crate::dialogs::Kind;
use crate::viewport::{Hit, Proj};

/// A drag under way.
#[derive(Clone, Debug)]
pub struct Drag {
    /// The plane the pointer moves the grain in: through where it was grabbed, across the grain
    /// direction the view looks along most.
    origin: Vec3,
    normal: Vec3,
    /// The body's grain directions (world): along the grain, across its width, across its
    /// thickness.
    axes: [Vec3; 3],
    /// The grain's place as the drag began: pith offset and shift along the grain, mm.
    pith: [f64; 2],
    shift: f64,
    /// The drag's undo step (it went in with the first frame that moved).
    step: Option<Arc<Document>>,
}

/// Is the Appearance dialog waiting for a grain drag?
pub fn armed(app: &SolveApp) -> bool {
    matches!(app.dialog.as_ref().map(|d| &d.kind), Some(Kind::Appearance(k)) if k.move_grain && k.grain.is_some())
}

pub fn active(app: &SolveApp) -> bool {
    app.viewport.grain_drag.is_some()
}

/// Begin a drag on the wooden body under `p`. True when one began.
pub fn start(app: &mut SolveApp, proj: &Proj, p: Pos2) -> bool {
    let Some(Kind::Appearance(k)) = app.dialog.as_ref().map(|d| &d.kind) else { return false };
    if !k.move_grain {
        return false;
    }
    let Some(grain) = k.grain.as_ref() else { return false };
    let number = |v: Option<&Value>| v.and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(0.0);
    let pith = match grain.get("pith_offset_mm") {
        Some(Value::Array(a)) => [number(a.first()), number(a.get(1))],
        other => [0.0, number(other)],
    };
    let shift = number(grain.get("shift_mm"));
    let Some((body, at)) = crate::viewport::pick(app, proj, p).into_iter().find_map(|h| match h {
        Hit::Face { body, point, .. } => Some((body, point)),
        _ => None,
    }) else {
        return false;
    };
    let state = crate::open_meshing::session_world(&app.session);
    let Some(shade) = state.body(&body).and_then(|b| app.session.doc.model_body_wood(b)) else { return false };
    let axes = shade.to_grain.map(|r| Vec3::new(f64::from(r[0]), f64::from(r[1]), f64::from(r[2])).normalized().unwrap_or(Vec3::X));
    // The grain moves in the plane of its own directions that faces the view most squarely: on a
    // board's face along and across the board, on its end across its width and thickness.
    let view = proj.ray(p).1;
    let normal = axes.into_iter().fold(axes[0], |best, axis| if view.dot(axis).abs() > view.dot(best).abs() { axis } else { best });
    app.viewport.grain_drag = Some(Drag { origin: at, normal, axes, pith, shift, step: None });
    true
}

/// Bring the grain grabbed to the pointer at `p`.
pub fn to(app: &mut SolveApp, proj: &Proj, p: Pos2) {
    let Some(d) = app.viewport.grain_drag.clone() else { return };
    // Something else took the drag's undo step (undo pressed mid-drag): the drag ends there.
    if let Some(doc) = &d.step
        && !app.session.undo.last().is_some_and(|step| Arc::ptr_eq(&step.doc, doc))
    {
        end(app);
        return;
    }
    let (o, dir) = proj.ray(p);
    let facing = dir.dot(d.normal);
    if facing.abs() < 1e-9 {
        return;
    }
    let moved = o + dir * ((d.origin - o).dot(d.normal) / facing) - d.origin;
    if !moved.is_finite() {
        return;
    }
    let place = |v: f64| ((v * 100.0).round() / 100.0).clamp(-MAX_PITH_MM, MAX_PITH_MM);
    let pith = [place(d.pith[0] + moved.dot(d.axes[1])), place(d.pith[1] + moved.dot(d.axes[2]))];
    let shift = place(d.shift + moved.dot(d.axes[0]));
    let Some(dialog) = app.dialog.as_mut() else {
        end(app);
        return;
    };
    let Kind::Appearance(k) = &mut dialog.kind else {
        end(app);
        return;
    };
    let Some(grain) = k.grain.as_mut().and_then(Value::as_object_mut) else {
        end(app);
        return;
    };
    grain.insert("pith_offset_mm".into(), json!(pith));
    grain.insert("shift_mm".into(), json!(shift));
    let items = dialog.inputs.first().map(|i| i.items.clone()).unwrap_or_default();
    let Some(params) = crate::dialogs_appearance::selected_params(k, &items) else { return };
    // One undo step for the whole drag: the first frame's, kept on top while the others run.
    let own = if d.step.is_some() { app.session.undo.pop() } else { None };
    let before = app.session.doc.clone();
    let ran = app.run("appearance.assign", params.clone()).is_ok();
    let pushed = ran && !Arc::ptr_eq(&before, &app.session.doc);
    match own {
        Some(step) => {
            if pushed {
                app.session.undo.pop();
            }
            app.session.undo.push(step);
        }
        None if pushed => {
            let step = app.session.undo.last().map(|step| step.doc.clone());
            if let Some(d) = app.viewport.grain_drag.as_mut() {
                d.step = step;
            }
        }
        None => {}
    }
    // OK has nothing left to do for what the drag already assigned.
    if ran && let Some(Kind::Appearance(k)) = app.dialog.as_mut().map(|d| &mut d.kind) {
        k.applied = Some(params);
        k.remove_override = false;
    }
}

/// Let go: the grain stays where it was dragged.
pub fn end(app: &mut SolveApp) {
    app.viewport.grain_drag = None;
}
