use std::io;
use std::rc::Rc;

use serde::Deserialize;
use serde_json;
use specs::{Component, ReadExpect, ReadStorage, System, VecStorage};
use specs::Join;

use crate::{asset, InitData, InsertInfo, math, Module, WgpuState, InitContext, WgpuStateCell};
use crate::asset::*;
use crate::resource::*;
use crate::client::graphics::{Material, Texture, ShaderProgram, UniformLayoutConfig, UniformBindingType, UniformPropertyBinding, UniformPropertyType, UniformVisibility, MatProperty, SamplerConfig};
use crate::client::graphics;
use crate::ecs::Transform;
use crate::math::*;
use crate::util::Color;
use crate::resource::{ResourceRef, ResManager};
use std::collections::HashMap;

#[derive(Clone, Deserialize)]
pub struct SpriteConfig {
    name: String,
    // https://serde.rs/remote-derive.html
    #[serde(with = "Vec2SerdeRef")]
    pos: Vec2,  // Center of the sprite, in pixel coordinates
    #[serde(with = "Vec2SerdeRef")]
    size: Vec2, // Size of the image, in pixel coordinates
    #[serde(with = "Vec2SerdeRef")]
    pivot: Vec2, // the pos of the pivot within the sprite (normalized 0-1 range)
}

#[derive(Deserialize)]
pub struct SpriteSheetConfig {
    texture: String,
    sprites: Vec<SpriteConfig>,
    ppu: u32,
    #[serde(skip)]
    _path: String,
}

impl LoadableAsset for SpriteSheetConfig {
    fn read(path: &str) -> io::Result<Self> {
        let text = asset::load_asset::<String>(path)?;
        let mut config: SpriteSheetConfig = serde_json::from_str(&text)?;

        config._path = asset::get_dir(path);
        Ok(config)
    }
}

#[derive(Clone)]
pub struct Sprite {
    pub config: SpriteConfig,
    pub uv_min: Vec2,
    pub uv_max: Vec2
}

pub struct SpriteSheet {
    pub sprites: Vec<Sprite>,
    pub texture: ResourceRef<Texture>,
    pub ppu: u32,
}

impl SpriteSheet {
    pub fn find_sprite(&self, name: &str) -> Option<(usize, &Sprite)> {
        self.sprites.iter().enumerate().find(|(_, x)| &x.config.name == name)
    }
}

// 由于texture不能放到Component里（无法跨线程），且太重量级，在加载完后以及Component层使用SpriteRef
// 在渲染时才由SpriteRef拿回Sprite，利用Texture进行实际绘制

#[derive(Clone)]
pub struct SpriteRef {
    pub sheet: ResourceRef<SpriteSheet>,
    pub idx: usize,
}

impl SpriteRef {

    pub fn from_name(res_mgr: &ResManager, sheet: &ResourceRef<SpriteSheet>, name: &str) -> Option<Self> {
        res_mgr.get(&sheet).find_sprite(name)
            .map(|(idx, _)| SpriteRef::new(&sheet, idx))
    }

    pub fn new(sheet: &ResourceRef<SpriteSheet>, idx: usize) -> Self {
        Self {
            sheet: sheet.clone(),
            idx
        }
    }
}

pub fn load_sprite_sheet(res_mgr: &mut ResManager, wgpu_state: &WgpuState, path: &str) -> io::Result<ResourceRef<SpriteSheet>> {
    let config: SpriteSheetConfig = asset::load_asset(path)?;
    let texture = graphics::load_texture(wgpu_state,
                                         &asset::get_asset_path_local(&config._path, &config.texture));
    let (tex_width, tex_height) = (texture.size.width as f32, texture.size.height as f32);

    let sprites: Vec<Sprite> = (&config.sprites).into_iter()
        .map(|x| {
            let tuv1: Vec2 = x.pos - x.size * 0.5;
            let tuv2: Vec2 = x.pos + x.size * 0.5;

            let u1 = tuv1.x / tex_width;
            let v1 = tuv2.y / tex_height;
            let u2 = tuv2.x / tex_width;
            let v2 = tuv1.y / tex_height;

            Sprite { config: x.clone(), uv_min: vec2(u1, v1), uv_max: vec2(u2, v2) }
        })
        .collect();

    let sheet = SpriteSheet {
        texture: res_mgr.add(texture),
        sprites,
        ppu: config.ppu,
    };

    Ok(res_mgr.add(sheet))
}

pub struct SpriteRenderer {
    pub sprite: SpriteRef,
    pub material: Option<ResourceRef<Material>>,
    pub color: Color
}

impl SpriteRenderer {

    pub fn new(spr: SpriteRef) -> Self {
        Self {
            sprite: spr,
            material: None,
            color: Color::white()
        }
    }

}

impl Component for SpriteRenderer {
    type Storage = VecStorage<Self>;
}

pub struct SpriteModule;

impl Module for SpriteModule {
    fn init(&self, init_context: &mut InitContext) {
        let display_clone = init_context.display.clone();
        init_context.dispatch_thread_local(
        InsertInfo::new("sprite")
                .before(&[graphics::DEP_RENDER_TEARDOWN])
                .after(&[graphics::DEP_RENDER_SETUP])
                .order(graphics::render_order::OPAQUE),
            move |f| f.insert_thread_local(SpriteRenderSystem::new(display_clone)));
    }
}

#[derive(Copy, Clone)]
struct SpriteVertex {
    v_pos: [f32; 2],
    v_uv: [f32; 2],
}

impl SpriteVertex {
    fn new(x: f32, y: f32, u: f32, v: f32) -> Self {
        SpriteVertex {
            v_pos: [x, y],
            v_uv: [u, v]
        }
    }
}

impl_vertex!(SpriteVertex, v_pos => 0, v_uv => 1);

#[derive(Copy, Clone, Default)]
struct SpriteInstanceData {
    i_mat_col0: [f32; 3],
    i_mat_col1: [f32; 3],
    i_mat_col2: [f32; 3],
    i_mat_col3: [f32; 3],
    i_uv_min: [f32; 2],
    i_uv_max: [f32; 2],
    i_color: [f32; 4]
}

impl_vertex!(SpriteInstanceData, Instance,
    i_mat_col0 => 2, i_mat_col1 => 3, i_mat_col2 => 4, i_mat_col3 => 5,
    i_uv_min => 6, i_uv_max => 7, i_color => 8);

#[derive(Copy, Clone)]
struct SpriteUniformData {
    pub mat: [f32; 16]
}

struct SpriteRenderSystem {
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    sprite_program: ResourceRef<ShaderProgram>,
    wgpu_state: WgpuStateCell,
    material: Option<Material>,
    pipeline: wgpu::RenderPipeline
}

impl SpriteRenderSystem {

    pub fn new(res_mgr: &mut ResManager, wgpu_state_cell: WgpuStateCell) -> Self {
        let wgpu_state = wgpu_state_cell.borrow();
        let vert = include_str!("../../assets/sprite_default.vert");
        let frag = include_str!("../../assets/sprite_default.frag");

        let program = graphics::load_shader_by_content(&*wgpu_state.device,
           vert, frag,
           "sprite_default.vert", "sprite_default.frag",
           &[
               UniformLayoutConfig {
                   binding: 0,
                   name: "".to_string(),
                   ty: UniformBindingType::DataBlock {
                       members: vec![
                           UniformPropertyBinding("u_proj".to_string(), UniformPropertyType::Mat4)
                       ]
                   },
                   visibility: UniformVisibility::Vertex
               },
               UniformLayoutConfig {
                   binding: 1,
                   name: "u_texture".to_string(),
                   ty: UniformBindingType::Texture,
                   visibility: UniformVisibility::Fragment
               },
               UniformLayoutConfig {
                   binding: 2,
                   name: "u_sampler".to_string(),
                   ty: UniformBindingType::Sampler,
                   visibility: UniformVisibility::Fragment
               },
           ]);
        let program_ref = res_mgr.add(program);
        let program = res_mgr.get(&program_ref);

        let vertices = [
            SpriteVertex::new(-0.5, -0.5, 0., 0.),
            SpriteVertex::new(-0.5, 0.5, 0., 1.),
            SpriteVertex::new(0.5, 0.5, 1., 1.),
            SpriteVertex::new(0.5, -0.5, 1., 0.)
        ];
        let vbo = wgpu_state.device.create_buffer_with_data(
            bytemuck::cast_slice(&[vertices]),
            wgpu::BufferUsage::VERTEX
        );

        let indices = [0u16, 1, 2, 0, 2, 3];
        let ibo = wgpu_state.device.create_buffer_with_data(
            bytemuck::cast(&ibo),
            wgpu::BufferUsage::INDEX
        );

        let pipeline_layout = wgpu_state.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&program.bind_group_layout]
        });

        let pipeline = wgpu_state.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: program.vertex_desc(),
            fragment_stage: Some(program.fragment_desc()),
            rasterization_state: None,
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[
                wgpu::ColorStateDescriptor {
                    format: wgpu_state.sc_desc.format,
                    alpha_blend: wgpu::BlendDescriptor::REPLACE,
                    color_blend: wgpu::BlendDescriptor::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL
                }
            ],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[get_vertex!(SpriteVertex), get_vertex!(SpriteInstanceData)]
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false
        });

        drop(wgpu_state);

        Self {
            vbo,
            ibo,
            sprite_program: program_ref,
            wgpu_state: wgpu_state_cell,
            material: None,
            pipeline
        }
    }

    fn _flush_current_batch(&mut self, res_mgr: &ResManager, batch: Batch) {
        let sheet = res_mgr.get(&batch.sheet);

        let instance_data = (&batch.sprites).iter()
            .map(|x| {
                let sprite_ref = &sheet.sprites[x.idx];

                let sprite_scl: Vec2 = sprite_ref.config.size / (sheet.ppu as f32);
                let sprite_offset: Vec2 = -(sprite_ref.config.pivot - math::vec2(0.5, 0.5));
                let world_view: Mat4 = x.world_view *
                    Mat4::from_nonuniform_scale(sprite_scl.x, sprite_scl.y, 1.0) *
                    Mat4::from_translation(sprite_offset.extend(0.0));

                #[inline]
                fn xyz(v: Vec4) -> [f32; 3] {
                    [v.x, v.y, v.z]
                }

                // sprite_ref.config.
                SpriteInstanceData {
                    i_mat_col0: xyz(world_view.x),
                    i_mat_col1: xyz(world_view.y),
                    i_mat_col2: xyz(world_view.z),
                    i_mat_col3: xyz(world_view.w),
                    i_uv_min: [sprite_ref.uv_min.x, sprite_ref.uv_min.y],
                    i_uv_max: [sprite_ref.uv_max.x, sprite_ref.uv_max.y],
                    i_color: x.color.into(),
                }
            })
            .collect::<Vec<_>>();

        let wgpu_state = self.wgpu_state.borrow();
        let instance_buf = wgpu_state.device.create_buffer_with_data(
            bytemuck::cast_slice(&instance_data),
            wgpu::BufferUsage::VERTEX
        );

        graphics::with_render_data(|r| {
            let camera_infos = &r.camera_infos;
            for cam in camera_infos {
                let wvp_mat: [f32; 16] = cam.wvp_matrix.into();

                let material = match &mut self.material {
                    Some(mat) => {
                        mat.set("u_texture", MatProperty::Texture(sheet.texture.clone()));
                        mat.set("u_sampler", MatProperty::TextureSampler(sheet.texture.clone()));
                    },
                    None => {
                        let mut properties = HashMap::new();
                        properties.insert("u_proj".to_string(), MatProperty::Mat4(cam.wvp_matrix));
                        properties.insert("u_texture".to_string(), MatProperty::Texture(sheet.texture.clone()));
                        properties.insert("u_sampler".to_string(), MatProperty::TextureSampler(sheet.texture.clone()));
                        self.material = Some(Material::create(
                            res_mgr,
                            &*wgpu_state,
                            self.sprite_program.clone(),
                            properties
                        ))
                    }
                };

                let texture = res_mgr.get(&sheet.texture);
                let uniforms = glium::uniform! {
                    u_proj: wvp_mat,
                    u_texture: &texture.raw_texture
                };

                if let Some(material) = &batch.material {
                    let program = res_mgr.get(&material.program);
                    let mat_uniforms = material.as_uniforms(&res_mgr);
                    let uniforms =
                        graphics::MaterialCombinedUniforms::new(uniforms, mat_uniforms);
                    frame.draw(
                        (&self.vbo, self.instance_buf.per_instance().unwrap()),
                        &self.ibo,
                        program,
                        &uniforms,
                        &Default::default()).unwrap();
                } else {
                    let program = res_mgr.get(&self.sprite_program);
                    frame.draw(
                        (&self.vbo, self.instance_buf.per_instance().unwrap()),
                        &self.ibo,
                        program,
                        &uniforms,
                        &Default::default()).unwrap();
                }
            }
        });
    }
}

struct SpriteInstance {
    world_view: Mat4,
    idx: usize,
    color: Color
}

struct Batch {
    sheet: ResourceRef<SpriteSheet>,
    sprites: Vec<SpriteInstance>,
    material: Option<ResourceRef<Material>>
}

impl<'a> System<'a> for SpriteRenderSystem {
    type SystemData = (ReadExpect<'a, ResManager>, ReadStorage<'a, SpriteRenderer>, ReadStorage<'a, Transform>);

    fn run(&mut self, (sprite_mgr, sr_vec, trans_vec): Self::SystemData) {
        let mut cur_batch: Option<Batch> = None;
        for (trans, sr) in (&trans_vec, &sr_vec).join() {
            let world_view: Mat4 = math::Mat4::from_translation(trans.pos) * Mat4::from(trans.rot);
            let sprite_instance = SpriteInstance {
                idx: sr.sprite.idx,
                world_view,
                color: sr.color.clone()
            };
            // Batching
            let cur_taken = cur_batch.take();
            // Has last batch
            if let Some(mut cur_taken) = cur_taken {
                // TODO: Add material difference telling
                if cur_taken.sheet == sr.sprite.sheet { // Can batch, add to list
                    cur_taken.sprites.push(sprite_instance);
                    cur_batch = Some(cur_taken);
                } else { // Can't batch, flush current && set now as now
                    self._flush_current_batch(&sprite_mgr, cur_taken);
                    cur_batch = Some(Batch {
                        sheet: sr.sprite.sheet.clone(),
                        sprites: vec![sprite_instance],
                        material: sr.material.clone() // FIXME: Useless clone
                    });
                }
            } else { // No previous batch, set one
                cur_batch = Some(Batch {
                    sheet: sr.sprite.sheet.clone(),
                    sprites: vec![sprite_instance],
                    material: sr.material.clone()
                });
            }
        }

        // Flush final batch
        if let Some(final_batch) = cur_batch.take() {
            self._flush_current_batch(&sprite_mgr, final_batch);
        }
    }

}
