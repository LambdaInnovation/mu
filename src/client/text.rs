use wgpu_glyph::*;
use std::collections::HashMap;
use crate::Module;
use crate::util::Color;

/// A `Resource` describing all fonts that will be used by
pub struct FontInitData {
    pub fonts: HashMap<String, ab_glyph::FontArc>
}

/// A `Resource` for runtime-initialized font data & glyph brush.
pub struct FontRuntimeData {
    pub fonts: HashMap<String, FontId>,
    pub glyph_brush: GlyphBrush<()>
}

/// A `Component` representing text rendering in world space. Attached on an entity with `Transform`
///  to take effect.
pub struct WorldText {
    pub text: String,
    pub color: Color,
}

pub struct TextModule;

impl Module for TextModule {

}