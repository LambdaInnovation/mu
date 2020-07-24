use crate::{asset, Module, InitData, InsertInfo, math};
use crate::math::*;
use crate::asset::{LoadableAsset, ResourceRef, ResourcePool, ResManager};
use crate::client::graphics::{Texture, Material};
use crate::client::graphics;
use crate::ecs::Transform;

use serde_json;
use serde::Deserialize;
use glium;
use glium::{Display, VertexBuffer, IndexBuffer, Surface, Program};
use specs::{Component, VecStorage, System, ReadStorage, ReadExpect};
use specs::Join;

use std::io;
use glium::index::PrimitiveType;
use std::rc::Rc;

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
}

pub struct SpriteSheet {
    pub sprites: Vec<Sprite>,
    pub texture: ResourceRef<Texture>,
    pub ppu: u32,
}

impl SpriteSheet {
    pub fn find_sprite(&self, name: &str) -> Option<&Sprite> {
        self.sprites.iter().find(|x| &x.config.name == name)
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
    pub fn new(sheet: ResourceRef<SpriteSheet>, idx: usize) -> Self {
        Self {
            sheet,
            idx
        }
    }
}

impl ResourcePool<SpriteSheet> {

    pub fn load(&mut self, display: &Display, path: &str) -> io::Result<ResourceRef<SpriteSheet>> {
        let config: SpriteSheetConfig = asset::load_asset(path)?;
        let texture = graphics::load_texture(display,
                                             &asset::get_asset_path_local(&config._path, &config.texture));

        let sprites: Vec<Sprite> = (&config.sprites).into_iter()
            .map(|x| Sprite { config: x.clone() })
            .collect();

        let sheet = SpriteSheet {
            texture,
            sprites,
            ppu: config.ppu,
        };

        Ok(self.add(sheet))
    }
}

pub struct SpriteRenderer {
    pub sprite: SpriteRef,
    pub material: Option<graphics::Material>
}

impl Component for SpriteRenderer {
    type Storage = VecStorage<Self>;
}

pub struct SpriteModule;

impl Module for SpriteModule {
    fn init(&self, init_data: &mut InitData) {
        let display_clone = init_data.display.clone();
        init_data.dispatch_thread_local(
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

glium::implement_vertex!(SpriteVertex, v_pos, v_uv);

#[derive(Copy, Clone, Default)]
struct SpriteInstanceData {
    i_world_view: [[f32; 4]; 4],
    i_uv_min: [f32; 2],
    i_uv_max: [f32; 2]
}

glium::implement_vertex!(SpriteInstanceData, i_world_view, i_uv_min, i_uv_max);

struct SpriteRenderSystem {
    vbo: VertexBuffer<SpriteVertex>,
    instance_buf: VertexBuffer<SpriteInstanceData>,
    ibo: IndexBuffer<u16>,
    sprite_program: ResourceRef<Program>,
    display: Rc<Display>
}

impl SpriteRenderSystem {

    pub fn new(display_rc: Rc<Display>) -> Self {
        let display = &*display_rc;
        let vert = include_str!("../../assets/sprite_default.vert");
        let frag = include_str!("../../assets/sprite_default.frag");
        let program = graphics::load_shader_by_content(&display, vert, frag);
        let vbo = VertexBuffer::new(display, &[
            SpriteVertex::new(-0.5, -0.5, 0., 0.),
            SpriteVertex::new(-0.5, 0.5, 0., 1.),
            SpriteVertex::new(0.5, 0.5, 1., 1.),
            SpriteVertex::new(0.5, -0.5, 1., 0.)
        ]).unwrap();
        let instance_buf: VertexBuffer<SpriteInstanceData> = VertexBuffer::dynamic(
            display, &[Default::default(); 4096])
            .unwrap();

        let ibo = IndexBuffer::new(display, PrimitiveType::TrianglesList,
                                   &[0u16, 1, 2, 0, 2, 3]).unwrap();

        Self {
            vbo,
            instance_buf,
            ibo,
            sprite_program: program,
            display: display_rc
        }
    }

    fn _flush_current_batch(&mut self, res_mgr: &ResManager, batch: Batch) {
        let sheet = res_mgr.get(&batch.sheet);

        let instance_data = (&batch.sprites).iter()
            .map(|x| {
                let sprite_ref = &sheet.sprites[x.idx];

                asset::with_local_resource_mgr(|res_mgr| {
                    let texture = res_mgr.get(&sheet.texture);
                    let tex_width = texture.raw_texture.width() as f32;
                    let tex_height = texture.raw_texture.height() as f32;

                    let tuv1: Vec2 = sprite_ref.config.pos - sprite_ref.config.size * 0.5;
                    let tuv2: Vec2 = sprite_ref.config.pos + sprite_ref.config.size * 0.5;

                    let u1 = tuv1.x / tex_width;
                    let v1 = tuv1.y / tex_height;
                    let u2 = tuv2.x / tex_width;
                    let v2 = tuv2.y / tex_height;

                    // sprite_ref.config.
                    SpriteInstanceData {
                        i_world_view: x.world_view.into(),
                        i_uv_min: [u1, v1],
                        i_uv_max: [u2, v2]
                    }
                })
            })
            .collect::<Vec<_>>();

        // if instance_data.len() != batch.sprites.len() {
        self.instance_buf = VertexBuffer::dynamic(&*self.display, &instance_data).unwrap();
        // } else {
        //     self.instance_buf.write(&instance_data);
        // }

        graphics::with_render_data(|r| {
            let camera_infos = &r.camera_infos;
            let frame = &mut r.frame;
            for cam in camera_infos {
                let wvp_mat: [[f32; 4]; 4] = cam.wvp_matrix.into();

                asset::with_local_resource_mgr(|res_mgr| {
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
                });
            }
        });
    }
}

struct SpriteInstance {
    world_view: Mat4,
    idx: usize
}

struct Batch {
    sheet: ResourceRef<SpriteSheet>,
    sprites: Vec<SpriteInstance>,
    material: Option<Material>
}

impl<'a> System<'a> for SpriteRenderSystem {
    type SystemData = (ReadExpect<'a, ResManager>, ReadStorage<'a, SpriteRenderer>, ReadStorage<'a, Transform>);

    fn run(&mut self, (sprite_mgr, sr_vec, trans_vec): Self::SystemData) {
        let mut cur_batch: Option<Batch> = None;
        for (trans, sr) in (&trans_vec, &sr_vec).join() {
            let world_view: Mat4 = math::Mat4::from_translation(trans.pos) * Mat4::from(trans.rot);
            let sprite_instance = SpriteInstance {
                idx: sr.sprite.idx,
                world_view
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
