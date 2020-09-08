use wgpu_glyph::*;
use std::collections::HashMap;
use specs::prelude::*;
use crate::*;
use crate::math::*;
use crate::client::graphics::*;
use crate::util::Color;
use crate::ecs::Transform;
use crate::asset::*;
use std::io::{Error, ErrorKind};

pub use ab_glyph::FontArc;

impl LoadableAsset for FontArc {
    fn read(path: &str) -> std::io::Result<Self> {
        let bytes: Vec<u8> = load_asset(path)?;
        FontArc::try_from_vec(bytes)
            .map_err(|f| Error::new(ErrorKind::InvalidData, f))
    }
}

/// A `Resource` describing all fonts that will be used after.
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
    pub layout: Layout<BuiltInLineBreaker>,
    pub sz: f32,
}

impl Component for WorldText {
    type Storage = VecStorage<Self>;
}


mod internal {
    use super::*;

    pub struct WorldTextRenderSystem {
        staging_belt: wgpu::util::StagingBelt
    }

    impl WorldTextRenderSystem {

        pub fn new() -> Self {
            Self {
                staging_belt: wgpu::util::StagingBelt::new(1024)
            }
        }

    }

    impl<'a> System<'a> for WorldTextRenderSystem {
        type SystemData = (ReadExpect<'a, WgpuState>, WriteExpect<'a, FontRuntimeData>, ReadStorage<'a, WorldText>, ReadStorage<'a, Transform>);

        fn run(&mut self, (wgpu_state, mut font_data, world_text_read, transform_read): Self::SystemData) {
            let ref mut glyph_brush = font_data.glyph_brush;

            // Recall the staging belt
            futures::executor::block_on(self.staging_belt.recall());

            with_render_data(|rd| {
                for cam in &mut rd.camera_infos {
                    for (text, trans) in (&world_text_read, &transform_read).join() {
                        let size_scl = 48.; // TOOD: 估算相机中字符大小 并正确设置px
                        glyph_brush.queue(Section {
                            screen_position: (0.0, 0.0),
                            text: vec![Text::new(&text.text)
                                .with_scale(size_scl)
                                .with_color(text.color)],
                            layout: text.layout,
                            .. Section::default()
                        });

                        let scl = text.sz / size_scl;
                        let scl_mat = Mat4::from_nonuniform_scale(scl, -scl, 1.);
                        let wvp_mat = cam.wvp_matrix * trans.get_world_view() * scl_mat;

                        glyph_brush.draw_queued_with_transform(&wgpu_state.device, &mut self.staging_belt, &mut cam.encoder,
                           &wgpu_state.frame_texture.as_ref().unwrap().output.view, mat::to_array(wvp_mat))
                            .unwrap();
                    }
                }
            });

            self.staging_belt.finish();
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
            |_, i| i.insert_thread_local(internal::WorldTextRenderSystem::new())
        );
    }

    fn start(&self, start_data: &mut StartContext) {
        let init_data = start_data.world.read_resource::<FontInitData>();
        let wgpu_state = start_data.world.read_resource::<WgpuState>();

        let v = init_data.fonts.iter().collect::<Vec<_>>();

        let glb = GlyphBrushBuilder::using_fonts(v.iter().map(|(_, f)| (*f).clone()).collect())
            .build(&wgpu_state.device, wgpu_state.sc_desc.format);
        let glb_ui = GlyphBrushBuilder::using_fonts(v.iter().map(|(_, f)| (*f).clone()).collect())
            .build(&wgpu_state.device, wgpu_state.sc_desc.format);

        let rt_data = FontRuntimeData {
            fonts: v.iter()
                .enumerate()
                .map(|(i, (n, _))| ((*n).clone(), FontId(i))).collect(),
            glyph_brush: glb,
            glyph_brush_ui: glb_ui
        };

        drop(init_data);
        drop(wgpu_state);
        start_data.world.insert(rt_data);
    }
}