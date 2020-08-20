use wgpu_glyph::*;
use std::collections::HashMap;
use specs::prelude::*;
use crate::client::graphics::*;
use crate::math::*;
use crate::{Module, WgpuStateCell, StartContext, InitContext, InsertInfo};
use crate::util::Color;
use crate::ecs::Transform;
use crate::asset::*;
use std::io::{Error, ErrorKind};

pub use ab_glyph::FontArc;

impl LoadableAsset for FontArc {
    fn read(path: &str) -> std::io::Result<Self> {
        let bytes: Vec<u8> = load_asset(path)?;
        FontArc::try_from_vec(bytes)
            .map_err(|f| Error::new(ErrorKind::InvalidData, "Invalid font"))
    }
}

/// A `Resource` describing all fonts that will be used by
pub struct FontInitData {
    pub fonts: HashMap<String, ab_glyph::FontArc>
}

/// A `Resource` for runtime-initialized font data & glyph brush.
pub struct FontRuntimeData {
    pub fonts: HashMap<String, FontId>,
    pub glyph_brush: GlyphBrush<()>,
    pub glyph_brush_ui: GlyphBrush<()>
}

/// A `Component` representing text rendering in world space. Attached on an entity with `Transform`
///  to take effect.
pub struct WorldText {
    pub text: String,
    pub color: Color,
}

impl Component for WorldText {
    type Storage = VecStorage<Self>;
}


mod internal {
    use super::*;

    pub struct WorldTextRenderSystem {
        pub wgpu_state: WgpuStateCell
    }

    impl<'a> System<'a> for WorldTextRenderSystem {
        type SystemData = (WriteExpect<'a, FontRuntimeData>, ReadStorage<'a, WorldText>, ReadStorage<'a, Transform>);

        fn run(&mut self, (mut font_data, world_text_read, transform_read): Self::SystemData) {
            let wgpu_state = self.wgpu_state.borrow();
            let ref mut glyph_brush = font_data.glyph_brush;

            with_render_data(|rd| {
                for cam in &mut rd.camera_infos {
                    for (text, trans) in (&world_text_read, &transform_read).join() {
                        glyph_brush.queue(Section {
                            screen_position: (10.0, 10.0),
                            text: vec![Text::new(&text.text).with_scale(24.)],
                            .. Section::default()
                        });

                        // glyph_brush.draw_queued_with_transform(&wgpu_state.device, &mut cam.encoder,
                        //                                        &wgpu_state.frame_texture.as_ref().unwrap().view, mat::to_array(cam.wvp_matrix));
                        glyph_brush.draw_queued(&wgpu_state.device, &mut cam.encoder, &wgpu_state.frame_texture.as_ref().unwrap().view,
                            wgpu_state.sc_desc.width, wgpu_state.sc_desc.height);
                        // glyph_brush.draw_queued_with_transform(&wgpu_state.device, &mut cam.encoder,
                        //                                        &wgpu_state.frame_texture.as_ref().unwrap().view, mat::to_array(Mat4::one()));
                    }
                }
            });
        }
    }

}

pub struct TextModule;

impl Module for TextModule {
    fn init(&self, ctx: &mut InitContext) {
        ctx.group_thread_local.dispatch(
            InsertInfo::new("")
                .after(&[DEP_CAM_DRAW_SETUP])
                .before(&[DEP_CAM_DRAW_TEARDOWN]),
            |d, i| i.insert_thread_local(internal::WorldTextRenderSystem {
                wgpu_state: d.wgpu_state.clone()
            })
        );
    }

    fn start(&self, start_data: &mut StartContext) {
        let init_data = start_data.world.read_resource::<FontInitData>();
        let wgpu_state = start_data.wgpu_state.borrow();

        let v = init_data.fonts.iter().collect::<Vec<_>>();

        let glb = GlyphBrushBuilder::using_fonts(v.iter().map(|(s, f)| (*f).clone()).collect())
            .build(&wgpu_state.device, wgpu_state.sc_desc.format);
        let glb_ui = GlyphBrushBuilder::using_fonts(v.iter().map(|(s, f)| (*f).clone()).collect())
            .build(&wgpu_state.device, wgpu_state.sc_desc.format);

        let rt_data = FontRuntimeData {
            fonts: v.iter()
                .enumerate()
                .map(|(i, (n, _))| ((*n).clone(), FontId(i))).collect(),
            glyph_brush: glb,
            glyph_brush_ui: glb_ui
        };

        drop(init_data);
        start_data.world.insert(rt_data);
    }
}