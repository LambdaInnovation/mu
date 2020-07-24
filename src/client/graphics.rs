use std::cell::RefCell;
use std::rc::Rc;

use glium::{Display, Frame, Program, Surface};
use glium::program::ProgramCreationInput;
use glium::texture::RawImage2d;
use image::GenericImageView;
use serde::Deserialize;
use serde_json;
use specs::prelude::*;

use crate::asset;
use crate::asset::{load_asset, load_asset_local, LoadableAsset, ResourceRef, LocalResManager};
use crate::client::WindowInfo;
use crate::ecs::Transform;
use crate::glium;
use crate::math::{Mat4, Vec3};
use crate::math;
use crate::Module;
use crate::util::Color;
use uuid::Uuid;
use std::collections::HashMap;
use glium::uniforms::{Uniforms, UniformValue};

pub struct Texture {
    pub uuid: Uuid,
    pub raw_texture: glium::texture::CompressedSrgbTexture2d
}

pub const DEP_RENDER_SETUP: &str = "render_setup";
pub const DEP_RENDER_TEARDOWN: &str = "render_teardown";

pub mod render_order {
    pub const OPAQUE: i32 = 0;
    pub const DEBUG_UI: i32 = 11000;
}

pub struct CamRenderData {
    pub wvp_matrix: Mat4,
    pub world_pos: Vec3,
}

pub struct FrameRenderData {
    pub frame: Frame,
    pub camera_infos: Vec<CamRenderData>,
}

#[derive(Deserialize)]
struct ShaderConfig {
    vertex: String,
    fragment: String,
    #[serde(skip)]
    _path: String,
}

#[derive(Deserialize)]
struct TextureConfig {
    image: String,
    #[serde(skip)]
    _path: String
}

impl LoadableAsset for ShaderConfig {
    fn read(path: &str) -> std::io::Result<Self> {
        let json_str = load_asset::<String>(path)?;
        let mut ret: ShaderConfig = serde_json::from_str(&json_str)?;
        ret._path = String::from(crate::asset::get_dir(path));

        Ok(ret)
    }
}

impl LoadableAsset for TextureConfig {
    fn read(path: &str) -> std::io::Result<Self> {
        let json_str = load_asset::<String>(path)?;
        let mut ret: TextureConfig = serde_json::from_str(&json_str)?;
        ret._path = String::from(crate::asset::get_dir(path));

        Ok(ret)
    }
}

thread_local!(
    static FRAME_RENDER_DATA: RefCell<Option<FrameRenderData>> = RefCell::new(None);
);

/// Acquire the render data reference in the closure,
/// and (presumably) do the rendering.
pub fn with_render_data<F>(f: F)
    where
        F: FnOnce(&mut FrameRenderData),
{
    FRAME_RENDER_DATA.with(|data| match *data.borrow_mut() {
        Some(ref mut data) => f(data),
        _ => panic!("No render data specified now"),
    });
}

pub fn load_shader(display: &Display, path: &str)
    -> ResourceRef<Program> {
    let config: ShaderConfig = load_asset(path).unwrap();
    let vert: String = crate::asset::load_asset_local(&config._path, &config.vertex).unwrap();
    let frag: String = crate::asset::load_asset_local(&config._path, &config.fragment).unwrap();
    load_shader_by_content(&display, &vert, &frag)
}

pub fn load_shader_by_content(display: &Display, vertex: &str, fragment: &str)
    -> ResourceRef<Program> {
    let program_input = ProgramCreationInput::SourceCode {
        vertex_shader: vertex,
        fragment_shader: fragment,
        tessellation_control_shader: None,
        tessellation_evaluation_shader: None,
        transform_feedback_varyings: None,
        geometry_shader: None,
        outputs_srgb: false,
        uses_point_size: false,
    };

    asset::add_local_resource(Program::new(&*display, program_input).unwrap())
}

pub fn load_texture(display: &Display, path: &str) -> ResourceRef<Texture> {
    let config: TextureConfig = load_asset(path).unwrap();
    let img_bytes: Vec<u8> = load_asset_local(&config._path, &config.image).unwrap();
    let img = image::load_from_memory_with_format(&img_bytes,
                                                  image::ImageFormat::Png).unwrap();
    let img_dims = img.dimensions();
    let img = RawImage2d::from_raw_rgba(img.into_rgba().into_vec(), img_dims);
    let raw_texture = glium::texture::CompressedSrgbTexture2d::new(display, img).unwrap();

    let ret = Texture {
        uuid: Uuid::new_v4(),
        raw_texture
    };
    asset::add_local_resource(ret)
}

fn init_render_data(data: FrameRenderData) {
    FRAME_RENDER_DATA.with(|ref_cell| {
        *ref_cell.borrow_mut() = Some(data);
    });
}

fn clear_render_data() -> FrameRenderData {
    FRAME_RENDER_DATA.with(|ref_cell| ref_cell.borrow_mut().take().unwrap())
}

pub enum CameraProjection {
    Perspective {
        fov: f32,
        z_near: f32,
        z_far: f32
    },
    Orthographic {
        size: f32,
        z_near: f32,
        z_far: f32
    }
}

pub struct Camera {
    pub projection: CameraProjection,
    pub clear_color: Option<Color>,
    pub clear_depth: bool
}

impl Component for Camera {
    type Storage = specs::VecStorage<Self>;
}

struct SysRenderPrepare {
    display: Rc<Display>,
}

pub struct SysRenderTeardown {}

impl<'a> System<'a> for SysRenderPrepare {
    type SystemData = (ReadExpect<'a, WindowInfo>, ReadStorage<'a, Camera>, ReadStorage<'a, Transform>);

    fn run(&mut self, (window_info, cameras, transforms): Self::SystemData) {
        let mut frame = self.display.draw();
        // Calculate wvp matrix
        let aspect: f32 = window_info.get_aspect_ratio();
        let cam_infos = {
            let mut res: Vec<CamRenderData> = vec![];
            for (cam, trans) in (&cameras, &transforms).join() {

                let projection = match cam.projection {
                    CameraProjection::Perspective { fov, z_near, z_far } => {
                        math::mat::perspective(crate::math::deg(fov), aspect, z_near, z_far)
                    }
                    CameraProjection::Orthographic { size, z_near, z_far } => {
                        let half_size = size / 2.;

                        math::mat::ortho(-aspect * half_size, aspect * half_size,
                            -half_size, half_size,
                            z_near, z_far)
                    }
                };
                // let perspective: Mat4 = crate::math::cgmath::perspective()
                //     .as_matrix()
                //     .clone();
                let rot = Mat4::from(trans.rot);

                //            rot[(3, 3)] = 1.0;
                let world_view: Mat4 = math::Mat4::from_translation(-trans.pos) * rot;

                let wvp_matrix = projection * world_view;
                match cam.clear_color {
                    Some(color) => frame.clear_color_srgb(color.r, color.g, color.b, color.a),
                    _ => (),
                }

                frame.clear_depth(1.0);

                res.push(CamRenderData {
                    wvp_matrix,
                    world_pos: trans.pos,
                });
            }
            res
        };

        self::init_render_data(FrameRenderData {
            frame,
            camera_infos: cam_infos,
        });
    }
}

impl<'a> System<'a> for SysRenderTeardown {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
        let render_data = self::clear_render_data();
        render_data.frame.finish().unwrap();
    }
}

#[derive(Clone)]
pub enum MatProperty {
    Float(f32),
    Mat4([[f32; 4]; 4]),
    Sampler(ResourceRef<Texture>)
}

#[derive(Clone)]
pub struct Material {
    pub program: ResourceRef<Program>,
    pub uniforms: HashMap<String, MatProperty>
}

impl Material {

    pub fn as_uniforms<'a>(&self, res_mgr: &'a LocalResManager) -> MaterialUniforms<'a> {
        let properties: HashMap<_, _> = self.uniforms.iter()
            .map(|(k, v)| {
                let uniform_value = match v {
                    MatProperty::Float(f) => UniformValue::Float(f.clone()),
                    MatProperty::Mat4(m) => UniformValue::Mat4(m.clone()),
                    MatProperty::Sampler(s) =>
                        UniformValue::CompressedSrgbTexture2d(&res_mgr.get(s).raw_texture, None)
                };
                (k.clone(), uniform_value)
            })
            .collect();

        MaterialUniforms {
            properties
        }
    }

}

#[derive(Clone)]
pub struct MaterialUniforms<'a> {
    properties: HashMap<String, UniformValue<'a>>
}

impl Material {

    pub fn new(program: ResourceRef<Program>) -> Self {
        Self {
            program,
            uniforms: HashMap::new()
        }
    }

}

impl MaterialUniforms<'_> {

    fn ref_visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut func: F) {
        for (k, v) in &self.properties {
            func(&k, v.clone());
        }
    }
}

impl Uniforms for MaterialUniforms<'_> {
    fn visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut func: F) {
        self.ref_visit_values(&mut func);
    }
}


pub struct MaterialCombinedUniforms<'a, A> where A: Uniforms {
    a: A,
    b: MaterialUniforms<'a>
}

impl<'a, A> MaterialCombinedUniforms<'a, A> where A: Uniforms {
    pub(crate) fn new(a: A, b: MaterialUniforms<'a>) -> Self {
        Self {
            a, b
        }
    }
}

impl<A> Uniforms for MaterialCombinedUniforms<'_, A> where A: Uniforms {
    fn visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut func: F) {
        self.b.ref_visit_values(&mut func);
        self.a.visit_values(func);
    }
}

pub struct GraphicsModule;

impl Module for GraphicsModule {
    fn init(&self, init_data: &mut crate::InitData) {
        use crate::InsertInfo;
        {
            let display_clone = init_data.display.clone();
            init_data.dispatch_thread_local(
                InsertInfo::new(DEP_RENDER_SETUP)
                    .before(&[DEP_RENDER_TEARDOWN])
                    .order(100),
                move |f| {
                    f.insert_thread_local(SysRenderPrepare {
                        display: display_clone,
                    })
                },
            );
        }
        init_data.dispatch_thread_local(
            InsertInfo::new(DEP_RENDER_TEARDOWN).after(&[DEP_RENDER_SETUP]),
            |f| f.insert_thread_local(SysRenderTeardown {}),
        );
    }

    // fn start(&self, _start_data: &mut crate::StartData) {}
}
