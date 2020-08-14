use std::cell::RefCell;

use image::GenericImageView;
use serde::{Serialize, Deserialize};
use serde_json;
use specs::prelude::*;

use crate::{WgpuState};
use crate::asset::*;
use crate::resource::*;
use crate::client::WindowInfo;
use crate::ecs::Transform;
use crate::math::{Mat4, Vec3, Vec2};
use crate::math;
use crate::Module;
use crate::util::Color;
use uuid::Uuid;
use std::collections::HashMap;
use shaderc::ShaderKind;
use std::io::Cursor;

pub const DEP_CAM_DRAW_SETUP: &str = "cam_draw_setup";
pub const DEP_CAM_DRAW_TEARDOWN: &str = "cam_draw_teardown";

pub mod render_order {
    pub const OPAQUE: i32 = 0;
    pub const UI: i32 = 1000;
    pub const DEBUG_UI: i32 = 11000;
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
    pub clear_depth: bool,
}

impl Component for Camera {
    type Storage = specs::VecStorage<Self>;
}

pub struct CamRenderData {
    pub entity: Entity,
    pub wvp_matrix: Mat4,
    pub world_pos: Vec3,
    pub encoder: wgpu::CommandEncoder,
}

impl CamRenderData {

    /// Creates a `RenderPass` for this camera.
    pub fn render_pass<'a>(&'a mut self, wgpu_state: &'a WgpuState) -> wgpu::RenderPass<'a> {
        let render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[
                wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &wgpu_state.frame_texture.as_ref().unwrap().view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Load,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: Default::default()
                }
            ],
            depth_stencil_attachment: None
        });

        render_pass
    }

}

pub struct FrameRenderData {
    pub camera_infos: Vec<CamRenderData>,
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

fn init_render_data(data: FrameRenderData) {
    FRAME_RENDER_DATA.with(|ref_cell| {
        *ref_cell.borrow_mut() = Some(data);
    });
}

fn clear_render_data() -> FrameRenderData {
    FRAME_RENDER_DATA.with(|ref_cell| ref_cell.borrow_mut().take().unwrap())
}

#[derive(Serialize, Deserialize)]
struct ShaderConfig {
    vertex: String,
    fragment: String,
    uniform_layout: Vec<UniformLayoutConfig>,
    #[serde(skip)]
    _path: String,
}

impl LoadableAsset for ShaderConfig {
    fn read(path: &str) -> std::io::Result<Self> {
        let json_str = load_asset::<String>(path)?;
        let mut ret: ShaderConfig = serde_json::from_str(&json_str)?;
        ret._path = String::from(crate::asset::get_dir(path));

        Ok(ret)
    }
}

pub struct ShaderProgram {
    pub vertex: wgpu::ShaderModule,
    pub fragment: wgpu::ShaderModule,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub layout_config: Vec<UniformLayoutConfig>
}

pub fn load_shader(device: &wgpu::Device, path: &str) -> ShaderProgram {
    let config: ShaderConfig = load_asset(path).unwrap();
    let vert: String = crate::asset::load_asset_local(&config._path, &config.vertex).unwrap();
    let frag: String = crate::asset::load_asset_local(&config._path, &config.fragment).unwrap();
    load_shader_by_content(device, &vert, &frag, &config.vertex, &config.fragment,
                           &config.uniform_layout)
}

pub fn load_shader_by_content(device: &wgpu::Device, vertex: &str, fragment: &str,
                              vert_filename: &str, frag_filename: &str, uniform_layout: &[UniformLayoutConfig])
                              -> ShaderProgram {

    let mut compiler = shaderc::Compiler::new()
        .expect("Can't create shader compiler");

    let vs_spirv = compiler.compile_into_spirv(vertex,
                                               ShaderKind::Vertex, "shader.vert", vert_filename, None).unwrap();
    let fs_spirv = compiler.compile_into_spirv(fragment,
                                               ShaderKind::Fragment, "shader.frag", frag_filename, None).unwrap();

    let vs_data = wgpu::read_spirv(Cursor::new(vs_spirv.as_binary_u8())).unwrap();
    let fs_data = wgpu::read_spirv(Cursor::new(fs_spirv.as_binary_u8())).unwrap();

    let vs_module = device.create_shader_module(&vs_data);
    let fs_module = device.create_shader_module(&fs_data);

    let label = format!("{}:{}", vert_filename, frag_filename);
    let descriptor = wgpu::BindGroupLayoutDescriptor {
        label: Some(&label), // TODO: Better label
        bindings: &uniform_layout.iter()
            .map(|x| {
                wgpu::BindGroupLayoutEntry {
                    binding: x.binding,
                    visibility: match &x.visibility {
                        UniformVisibility::Fragment => wgpu::ShaderStage::FRAGMENT,
                        UniformVisibility::Vertex => wgpu::ShaderStage::VERTEX,
                        UniformVisibility::All => wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT
                    },
                    ty: match &x.ty {
                        UniformBindingType::DataBlock { .. } => wgpu::BindingType::UniformBuffer {
                            dynamic: false
                        },
                        UniformBindingType::Texture => wgpu::BindingType::SampledTexture {
                            dimension: wgpu::TextureViewDimension::D2,
                            component_type: wgpu::TextureComponentType::Uint,
                            multisampled: false
                        },
                        UniformBindingType::Sampler => wgpu::BindingType::Sampler {
                            comparison: false
                        },
                    }
                }
            })
            .collect::<Vec<_>>(),
    };

    let shader_program = ShaderProgram {
        vertex: vs_module,
        fragment: fs_module,
        bind_group_layout: device.create_bind_group_layout(&descriptor),
        layout_config: uniform_layout.iter().map(|x| x.clone()).collect()
    };

    shader_program
}

#[derive(Serialize, Deserialize)]
struct TextureConfig {
    image: String,
    #[serde(skip)]
    _path: String
}

impl LoadableAsset for TextureConfig {
    fn read(path: &str) -> std::io::Result<Self> {
        let json_str = load_asset::<String>(path)?;
        let mut ret: TextureConfig = serde_json::from_str(&json_str)?;
        ret._path = String::from(crate::asset::get_dir(path));

        Ok(ret)
    }
}

pub struct Texture {
    pub uuid: Uuid,
    pub size: wgpu::Extent3d,
    pub raw_texture: wgpu::Texture
}

pub fn load_texture(wgpu_state: &WgpuState, path: &str) -> Texture {
    let config: TextureConfig = load_asset(path).unwrap();
    let img_bytes: Vec<u8> = load_asset_local(&config._path, &config.image).unwrap();
    let img = image::load_from_memory_with_format(&img_bytes,
                                                  image::ImageFormat::Png).unwrap();
    let img_dims = img.dimensions();
    create_texture(wgpu_state, img.into_rgba().into_vec(), img_dims)
}

pub fn create_texture(wgpu_state: &WgpuState, rgba_bytes: Vec<u8>, dims: (u32, u32)) -> Texture {
    let extent = wgpu::Extent3d {
        width: dims.0,
        height: dims.1,
        depth: 1,
    };
    let raw_texture = wgpu_state.device.create_texture(&wgpu::TextureDescriptor {
        size: extent,
        array_layer_count: 1,
        mip_level_count: 1, // TODO: mipmap
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST, // TODO: Read/Write
        label: Some("texture")
    });

    let buffer = wgpu_state.device.create_buffer_with_data(
        &rgba_bytes,
        wgpu::BufferUsage::COPY_SRC
    );

    let mut encoder = wgpu_state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("texture_upload_encoder")
    });

    encoder.copy_buffer_to_texture(
        wgpu::BufferCopyView {
            buffer: &buffer,
            offset: 0,
            bytes_per_row: 4 * dims.0,
            rows_per_image: dims.1
        },
        wgpu::TextureCopyView {
            texture: &raw_texture,
            mip_level: 0,
            array_layer: 0,
            origin: wgpu::Origin3d::ZERO
        },
        extent
    );

    wgpu_state.queue.submit(&[encoder.finish()]);

    let ret = Texture {
        uuid: Uuid::new_v4(),
        raw_texture,
        size: extent
    };
    ret
}

pub type UniformMat4 = [f32; 16];
pub type UniformMat3 = [f32; 9];

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum UniformPropertyType {
    Float, Vec2, Vec3, Mat4
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UniformPropertyBinding (
    String,
    UniformPropertyType
);

pub enum UniformProperty {
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    Mat4(Mat4)
}

#[derive(Clone, Serialize, Deserialize)]
pub enum UniformBindingType {
    Texture,
    Sampler,
    DataBlock { members: Vec<UniformPropertyBinding> }
}

pub enum UniformBinding {
    Texture(ResourceRef<Texture>),
    Sampler(ResourceRef<wgpu::Sampler>),
    DataBlock {
        members: Vec<UniformProperty>
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum UniformVisibility {
    Vertex,
    Fragment,
    All
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UniformLayoutConfig {
    pub binding: u32,
    #[serde(default)]
    pub name: String,
    pub ty: UniformBindingType,
    pub visibility: UniformVisibility
}

#[derive(Clone)]
pub enum MatProperty {
    Float(f32),
    Mat4([[f32; 4]; 4]),
    Sampler(ResourceRef<Texture>)
}

#[derive(Clone)]
pub struct Material {
    pub program: ResourceRef<ShaderProgram>,
    pub uniforms: HashMap<String, MatProperty>
}

impl Material {

    // pub fn as_uniforms<'a>(&self, res_mgr: &'a LocalResManager) -> MaterialUniforms<'a> {
    //     let properties: HashMap<_, _> = self.uniforms.iter()
    //         .map(|(k, v)| {
    //             let uniform_value = match v {
    //                 MatProperty::Float(f) => UniformValue::Float(f.clone()),
    //                 MatProperty::Mat4(m) => UniformValue::Mat4(m.clone()),
    //                 MatProperty::Sampler(s) =>
    //                     UniformValue::CompressedSrgbTexture2d(&res_mgr.get(s).raw_texture, None)
    //             };
    //             (k.clone(), uniform_value)
    //         })
    //         .collect();
    //
    //     MaterialUniforms {
    //         properties
    //     }
    // }

}

// #[derive(Clone)]
// pub struct MaterialUniforms<'a> {
//     properties: HashMap<String, UniformValue<'a>>
// }

impl Material {

    pub fn new(program: ResourceRef<ShaderProgram>) -> Self {
        Self {
            program,
            uniforms: HashMap::new()
        }
    }

}

mod internal {
    use super::*;
    use crate::WgpuStateCell;

    pub struct SysRenderPrepare {
        pub wgpu_state: WgpuStateCell,
    }

    pub struct SysRenderTeardown {
        pub wgpu_state: WgpuStateCell
    }

    impl<'a> System<'a> for SysRenderPrepare {
        type SystemData = (ReadExpect<'a, WindowInfo>, Entities<'a>, ReadStorage<'a, Camera>, ReadStorage<'a, Transform>);

        fn run(&mut self, (window_info, entities, cameras, transforms): Self::SystemData) {
            let wgpu_state = self.wgpu_state.borrow();
            // let mut frame = self.display.draw();
            // Calculate wvp matrix
            let aspect: f32 = window_info.get_aspect_ratio();

            let mut cam_id = 0;
            let mut result_vec = vec![];
            for (ent, cam, trans) in (&entities, &cameras, &transforms).join() {
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

                let mut encoder = wgpu_state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some(&format!("Camera {}", cam_id)),
                });

                let wvp_matrix = projection * world_view;
                match cam.clear_color {
                    Some(color) => {
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            color_attachments: &[
                                wgpu::RenderPassColorAttachmentDescriptor {
                                    attachment: &wgpu_state.frame_texture.as_ref().unwrap().view,
                                    resolve_target: None,
                                    load_op: wgpu::LoadOp::Clear,
                                    store_op: wgpu::StoreOp::Store,
                                    clear_color: color.into()
                                }
                            ],
                            depth_stencil_attachment: None
                        });
                    }
                    _ => (),
                }

                let cam_render_data = CamRenderData {
                    wvp_matrix,
                    world_pos: trans.pos,
                    encoder,
                    entity: ent,
                };

                result_vec.push(cam_render_data);
                cam_id += 1;
            }

            init_render_data(FrameRenderData {
                camera_infos: result_vec
            })
        }
    }

    impl<'a> System<'a> for SysRenderTeardown {
        type SystemData = ();

        fn run(&mut self, _: Self::SystemData) {
            let result = clear_render_data();
            let wgpu_state = self.wgpu_state.borrow();
            wgpu_state.queue.submit(
                &result.camera_infos
                    .into_iter()
                    .map(|x| x.encoder.finish())
                    .collect::<Vec<_>>())
        }
    }
}

// impl MaterialUniforms<'_> {
//
//     fn ref_visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut func: F) {
//         for (k, v) in &self.properties {
//             func(&k, v.clone());
//         }
//     }
// }
//
// impl Uniforms for MaterialUniforms<'_> {
//     fn visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut func: F) {
//         self.ref_visit_values(&mut func);
//     }
// }
//
//
// pub struct MaterialCombinedUniforms<'a, A> where A: Uniforms {
//     a: A,
//     b: MaterialUniforms<'a>
// }
//
// impl<'a, A> MaterialCombinedUniforms<'a, A> where A: Uniforms {
//     pub(crate) fn new(a: A, b: MaterialUniforms<'a>) -> Self {
//         Self {
//             a, b
//         }
//     }
// }

// impl<A> Uniforms for MaterialCombinedUniforms<'_, A> where A: Uniforms {
//     fn visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut func: F) {
//         self.b.ref_visit_values(&mut func);
//         self.a.visit_values(func);
//     }
// }

pub struct GraphicsModule;

impl Module for GraphicsModule {
    fn init(&self, init_data: &mut crate::InitContext) {
        use crate::InsertInfo;
        {
            init_data.dispatch_thread_local(
                InsertInfo::new(DEP_CAM_DRAW_SETUP)
                    .before(&[DEP_CAM_DRAW_TEARDOWN])
                    .order(100),
                move |init_data, f| {
                    f.insert_thread_local(internal::SysRenderPrepare {
                        wgpu_state: init_data.wgpu_state.clone()
                    })
                },
            );
        }
        init_data.dispatch_thread_local(
            InsertInfo::new(DEP_CAM_DRAW_TEARDOWN).after(&[DEP_CAM_DRAW_SETUP]),
            |init_data, f| f.insert_thread_local(internal::SysRenderTeardown {
                wgpu_state: init_data.wgpu_state.clone()
            }),
        );
    }

    // fn start(&self, _start_data: &mut crate::StartData) {}
}
