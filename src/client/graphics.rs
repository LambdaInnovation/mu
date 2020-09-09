use std::cell::RefCell;

use image::{GenericImageView, DynamicImage};
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
use imgui_inspect_derive::Inspect;
use crate::client::editor::asset_editor::{AssetInspectorResources, SerializeConfigInspectorFactory};
use crate::client::editor::inspect::*;
use strum_macros::*;
use imgui::*;
use imgui_inspect::*;
use wgpu::util::DeviceExt;

pub const DEP_CAM_DRAW_SETUP: &str = "cam_draw_setup";
pub const DEP_CAM_DRAW_TEARDOWN: &str = "cam_draw_teardown";

pub mod render_order {
    pub const OPAQUE: i32 = 0;
    pub const UI: i32 = 1000;
    pub const DEBUG_UI: i32 = 11000;
}

pub trait HasVertexFormat {
    fn format() -> wgpu::VertexFormat;
}

impl HasVertexFormat for f32 {
    fn format() -> wgpu::VertexFormat {
        wgpu::VertexFormat::Float
    }
}

impl HasVertexFormat for [f32; 2] {
    fn format() -> wgpu::VertexFormat {
        wgpu::VertexFormat::Float2
    }
}

impl HasVertexFormat for [f32; 3] {
    fn format() -> wgpu::VertexFormat {
        wgpu::VertexFormat::Float3
    }
}

impl HasVertexFormat for [f32; 4] {
    fn format() -> wgpu::VertexFormat {
        wgpu::VertexFormat::Float4
    }
}

pub fn __vertex_format<T>(_: &Option<&T>) -> wgpu::VertexFormat where T: HasVertexFormat {
    T::format()
}

pub fn __size_of<T>(_: &Option<&T>) -> wgpu::BufferAddress {
    std::mem::size_of::<T>() as wgpu::BufferAddress
}

#[macro_export]
macro_rules! impl_vertex {
    ($struct_name:ident, $step_mode:ident, $($field_name:ident => $field_location:expr), +) => {
        unsafe impl $crate::bytemuck::Pod for $struct_name {}
        unsafe impl $crate::bytemuck::Zeroable for $struct_name {}

        impl $struct_name {

            pub fn get_vertex_buffer_desc<'a>(v: &'a Vec<$crate::wgpu::VertexAttributeDescriptor>)
                -> $crate::wgpu::VertexBufferDescriptor<'a> {
                use std::mem::size_of;

                wgpu::VertexBufferDescriptor {
                    stride: size_of::<$struct_name>() as wgpu::BufferAddress,
                    step_mode: $crate::wgpu::InputStepMode::$step_mode,
                    attributes: &v
                }
            }

            pub fn get_vertex_attr_array() -> Vec<$crate::wgpu::VertexAttributeDescriptor> {
                let mut attrs = vec![];
                let mut bytes_sum = 0 as $crate::wgpu::BufferAddress;

                $(
                    let field_opt = None::<&$struct_name>.map(|v| &v.$field_name);
                    let len = $crate::client::graphics::__size_of(&field_opt);
                    bytes_sum += len;
                    attrs.push($crate::wgpu::VertexAttributeDescriptor {
                        offset: bytes_sum - len,
                        shader_location: $field_location,
                        format: {
                            $crate::client::graphics::__vertex_format(&field_opt)
                        }
                    });
                )+
                attrs
            }

        }
    };
    ($struct_name:ident, $($field_name:ident => $field_location:expr), +) => {
        $crate::impl_vertex!($struct_name, Vertex, $($field_name => $field_location),+);
    }
}

#[macro_export]
macro_rules! get_vertex {
    ($struct_name:ident) => {
        $struct_name::get_vertex_buffer_desc(&$struct_name::get_vertex_attr_array());
    }
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
                    attachment: &wgpu_state.frame_texture.as_ref().unwrap().output.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true
                    }
                }
            ],
            depth_stencil_attachment: None
        });

        render_pass
    }

}

pub struct FrameRenderData {
    pub camera_infos: Vec<CamRenderData>,
    /// A `StagingBelt` currently used for text rendering. Will be finished before all
    /// camera submission and recalled after that.
    pub staging_belt: wgpu::util::StagingBelt
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

#[derive(Serialize, Deserialize, Default)]
#[derive(Inspect)]
struct ShaderConfig {
    vertex: String,
    fragment: String,
    #[inspect(proxy_type="VecDefaultInspect<_>")]
    uniform_layout: Vec<UniformLayoutConfig>,
    #[serde(skip)]
    #[inspect(skip)]
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

impl ShaderProgram {

    pub fn vertex_desc(&self) -> wgpu::ProgrammableStageDescriptor {
        wgpu::ProgrammableStageDescriptor {
            module: &self.vertex,
            entry_point: "main"
        }
    }

    pub fn fragment_desc(&self) -> wgpu::ProgrammableStageDescriptor {
        wgpu::ProgrammableStageDescriptor {
            module: &self.fragment,
            entry_point: "main"
        }
    }

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

    let vs_data = wgpu::util::make_spirv(vs_spirv.as_binary_u8());
    let fs_data = wgpu::util::make_spirv(fs_spirv.as_binary_u8());

    let vs_module = device.create_shader_module(vs_data);
    let fs_module = device.create_shader_module(fs_data);

    let label = format!("{}:{}", vert_filename, frag_filename);
    let descriptor = wgpu::BindGroupLayoutDescriptor {
        label: Some(&label), // TODO: Better label
        entries: &uniform_layout.iter()
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
                            dynamic: false,
                            // TODO: Take advantage of the validation functionality
                            min_binding_size: None
                        },
                        UniformBindingType::Texture => wgpu::BindingType::SampledTexture {
                            dimension: wgpu::TextureViewDimension::D2,
                            component_type: wgpu::TextureComponentType::Uint,
                            multisampled: false
                        },
                        UniformBindingType::Sampler => wgpu::BindingType::Sampler {
                            comparison: false
                        },
                    },
                    count: None
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

#[derive(Serialize, Deserialize, PartialEq)]
#[derive(EnumIter, IntoStaticStr)]
pub enum FilterMode {
    Nearest,
    Bilinear,
    // TODO: Trilinear & mipmap
}

pub struct WgpuAddressModeInspect;

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
#[derive(EnumIter, IntoStaticStr)]
pub enum AddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
}

impl Default for AddressMode {
    fn default() -> Self { AddressMode::ClampToEdge }
}

impl Into<wgpu::AddressMode> for AddressMode {
    fn into(self) -> wgpu::AddressMode {
        match self {
            AddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            AddressMode::Repeat => wgpu::AddressMode::Repeat,
            AddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat
        }
    }
}

#[derive(Serialize, Deserialize, Inspect)]
pub struct SamplerConfig {
    #[serde(default)]
    #[inspect(proxy_type="EnumComboInspect<_>")]
    pub address: AddressMode,
    #[inspect(proxy_type="EnumComboInspect<_>")]
    pub filter: FilterMode
}

fn create_sampler_from_config(device: &wgpu::Device, cfg: &SamplerConfig) -> wgpu::Sampler {
    let (min_filter, mag_filter, mipmap_filter) = match cfg.filter {
        FilterMode::Nearest => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest),
        FilterMode::Bilinear => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear, wgpu::FilterMode::Nearest),
    };

    let wgpu_address = cfg.address.into();
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: None,
        address_mode_u: wgpu_address,
        address_mode_v: wgpu_address,
        address_mode_w: wgpu_address,
        min_filter, mag_filter, mipmap_filter,
        lod_min_clamp: 0.,
        lod_max_clamp: 0.,
        compare: None,
        anisotropy_clamp: None
    })
}

#[derive(Serialize, Deserialize, Inspect)]
pub struct TextureConfig {
    image: String,
    sampler: SamplerConfig,
    #[serde(skip)]
    #[inspect(skip)]
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
    pub raw_texture: wgpu::Texture,
    pub default_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler
}

pub fn load_texture_raw(path: &str) -> (TextureConfig, DynamicImage) {
    let config: TextureConfig = load_asset(path).unwrap();
    let img_bytes: Vec<u8> = load_asset_local(&config._path, &config.image).unwrap();
    let img = image::load_from_memory_with_format(&img_bytes,
                                                  image::ImageFormat::Png).unwrap();
    (config, img)
}

pub fn load_texture(wgpu_state: &WgpuState, path: &str) -> Texture {
    let config: TextureConfig = load_asset(path).unwrap();
    let img_bytes: Vec<u8> = load_asset_local(&config._path, &config.image).unwrap();
    let img = image::load_from_memory_with_format(&img_bytes,
                                                  image::ImageFormat::Png).unwrap();
    let img_dims = img.dimensions();
    create_texture(wgpu_state, img.into_rgba().into_vec(), img_dims, &config.sampler)
}

pub fn create_texture(wgpu_state: &WgpuState, rgba_bytes: Vec<u8>, dims: (u32, u32), sampler_cfg: &SamplerConfig) -> Texture {
    let extent = wgpu::Extent3d {
        width: dims.0,
        height: dims.1,
        depth: 1,
    };
    let raw_texture = wgpu_state.device.create_texture(&wgpu::TextureDescriptor {
        size: extent,
        mip_level_count: 1, // TODO: mipmap
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST, // TODO: Read/Write
        label: Some("texture")
    });

    wgpu_state.queue.write_texture(
        wgpu::TextureCopyView {
            texture: &raw_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO
        },
        &rgba_bytes,
        wgpu::TextureDataLayout {
            offset: 0,
            bytes_per_row: 4 * dims.0,
            rows_per_image: dims.1
        },
        extent
    );

    let default_view = raw_texture.create_view(&Default::default());
    let sampler = create_sampler_from_config(&wgpu_state.device, sampler_cfg);

    let ret = Texture {
        uuid: Uuid::new_v4(),
        raw_texture,
        size: extent,
        default_view,
        sampler
    };
    ret
}

pub type UniformMat4 = [f32; 16];
pub type UniformMat3 = [f32; 9];

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
#[derive(EnumIter, IntoStaticStr)]
pub enum UniformPropertyType {
    Float, Vec2, Vec3, Mat4
}

impl Default for UniformPropertyType {
    fn default() -> Self {
        Self::Float
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct UniformPropertyBinding (
    pub String,
    pub UniformPropertyType
);

impl InspectRenderDefault<UniformPropertyBinding> for UniformPropertyBinding {
    fn render(_: &[&UniformPropertyBinding], _: &'static str, _: &Ui, _: &InspectArgsDefault) {
        unimplemented!()
    }

    fn render_mut(data: &mut [&mut UniformPropertyBinding], label: &'static str, ui: &Ui, _args: &InspectArgsDefault) -> bool {
        let mut changed = false;

        let binding = &mut data[0];
        let mut name = im_str!("{}", binding.0);

        ui.columns(2, &im_str!("{}", label), false);
        if ui.input_text(im_str!("id"), &mut name).build() {
            changed = true;
            binding.0 = name.to_string();
        }
        ui.next_column();

        changed |= EnumComboInspect::render_mut(&mut [&mut binding.1], "type", ui, &Default::default());
        ui.next_column();
        ui.columns(1, im_str!(""), false);

        changed
    }
}

pub enum UniformProperty {
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    Mat4(Mat4)
}

impl UniformPropertyType {

    #[inline]
    fn element_count(&self) -> usize {
        match &self {
            Self::Float => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Mat4 => 16
        }
    }

}

#[derive(Clone, Serialize, Deserialize)]
pub enum UniformBindingType {
    Texture,
    Sampler,
    DataBlock { members: Vec<UniformPropertyBinding> }
}

impl Default for UniformBindingType {
    fn default() -> Self {
        Self::Texture
    }
}

impl InspectRenderDefault<UniformBindingType> for UniformBindingType {
    fn render(_: &[&UniformBindingType], _: &'static str, _: &Ui, _args: &InspectArgsDefault) {
        unimplemented!()
    }

    fn render_mut(data: &mut [&mut UniformBindingType], label: &'static str, ui: &Ui, _args: &InspectArgsDefault) -> bool {
        let item = &mut data[0];

        let mut ix = match item {
            UniformBindingType::Texture => 0,
            UniformBindingType::Sampler => 1,
            UniformBindingType::DataBlock { .. } => 2
        };

        let names = [
            im_str!("Texture"),
            im_str!("Sampler"),
            im_str!("DataBlock")
        ];

        if ComboBox::new(&im_str!("{}", label))
            .build_simple_string(ui, &mut ix, &names) {
            let new_item = match ix {
                0 => UniformBindingType::Texture,
                1 => UniformBindingType::Sampler,
                2 => UniformBindingType::DataBlock { members: Default::default() },
                _ => panic!("Invalid idx"),
            };
            **item = new_item;

            return true
        }

        if let UniformBindingType::DataBlock { members } = item {
            return VecDefaultInspect::<_>::render_mut(&mut [members], "Members", ui, &Default::default());
        }

        false
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
#[derive(EnumIter, IntoStaticStr)]
pub enum UniformVisibility {
    Vertex,
    Fragment,
    All
}

impl Default for UniformVisibility {
    fn default() -> Self {
        Self::Vertex
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[derive(Inspect)]
pub struct UniformLayoutConfig {
    pub binding: u32,
    #[serde(default)]
    pub name: String,
    pub ty: UniformBindingType,
    #[inspect(proxy_type="EnumComboInspect<_>")]
    pub visibility: UniformVisibility
}

#[derive(Clone)]
pub enum MatProperty {
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    Mat4(Mat4),
    Texture(ResourceRef<Texture>),
    TextureSampler(ResourceRef<Texture>),
    Sampler(ResourceRef<wgpu::Sampler>)
}

pub struct Material {
    pub program: ResourceRef<ShaderProgram>,
    pub properties: HashMap<String, MatProperty>,
    bind_group: wgpu::BindGroup,
    dirty: bool
}

impl Material {

    pub fn get_bind_group(&mut self, res_mgr: &ResManager, device: &wgpu::Device) -> &wgpu::BindGroup {
        if self.dirty {
            let program = res_mgr.get(&self.program);
            self.bind_group = Self::create_bind_group(res_mgr, program, device, &self.properties);
            self.dirty = false;
        }

        &self.bind_group
    }

    pub fn set(&mut self, name: &str, p: MatProperty) {
        assert!(self.properties.contains_key(name), "Can't add non-existent property");
        self.properties.insert(name.to_string(), p);
        self.mark_dirty();
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn create_bind_group<'a>(
        res_mgr: &'a ResManager,
        program: &ShaderProgram,
        device: &wgpu::Device,
        dict: &'a HashMap<String, MatProperty>) -> wgpu::BindGroup {
        let layout = &program.layout_config;

        enum FillEntry<'a> {
            DataBlock(u32, Vec<f32>, u32, Option<wgpu::Buffer>),
            Property(u32, Option<&'a MatProperty>),
        }

        enum FillKey {
            DataBlock(usize, usize, usize), // index, property index, float offset
            Property(usize)
        }

        let mut mapping: HashMap<String, FillKey> = HashMap::new();
        for (idx, elem) in layout.iter().enumerate() {
            match &elem.ty {
                UniformBindingType::Sampler | UniformBindingType::Texture => {
                    // info!("Add Property {}", elem.name);
                    if elem.name.len() > 0 {
                        mapping.insert(elem.name.clone(), FillKey::Property(idx));
                    }
                },
                UniformBindingType::DataBlock { members } => {
                    assert_eq!(elem.name.len(), 0, "DataBlock name is useless: {}", elem.name);
                    let mut sum = 0;
                    for (idx2, mem) in members.iter().enumerate() {
                        // info!("Add {}", mem.0);
                        mapping.insert(mem.0.clone(), FillKey::DataBlock(idx, idx2, sum));
                        sum += mem.1.element_count();
                    }
                }
            }
        }

        let mut data_vec = layout
            .iter()
            .map(|layout| {
                match &layout.ty {
                    UniformBindingType::Sampler | UniformBindingType::Texture => FillEntry::Property(layout.binding, None),
                    UniformBindingType::DataBlock { members } => {
                        let floats= vec![0.0;
                                         members.iter().map(|x| x.1.element_count()).sum()];
                        FillEntry::DataBlock(layout.binding, floats, 0, None)
                    }
                }
            })
            .collect::<Vec<_>>();

        for (k, v) in dict {
            let fill_key = mapping.get(k)
                .expect(&format!("No property named {} specified in config", &k));

            match fill_key {
                FillKey::Property(idx) => {
                    if let FillEntry::Property(_, p) = &mut data_vec[*idx] {
                        *p = Some(v);
                    } else {
                        panic!("Invalid property type for {}", k);
                    }
                },
                FillKey::DataBlock(ix, ix2, offset) => {
                    if let FillEntry::DataBlock(_, floats, flags, _) = &mut data_vec[*ix] {
                        *flags = *flags | (1 << (*ix2) as u32);

                        let slice = &mut floats.as_mut_slice()[*offset..];
                        match v {
                            MatProperty::Float(f) => slice[0] = *f,
                            MatProperty::Vec2(v) => {
                                slice[0] = v.x;
                                slice[1] = v.y;
                            },
                            MatProperty::Vec3(v) => {
                                slice[0] = v.x;
                                slice[1] = v.y;
                            },
                            MatProperty::Mat4(v) => {
                                let arr: [f32; 16] = math::mat::to_array(*v);
                                for i in 0..16 {
                                    slice[i] = arr[i];
                                }
                            }
                            _ => panic!("Invalid property type for {}", k)
                        }

                    } else {
                        panic!("Invalid property type for {}", k);
                    }
                }
            }
        }

        for (i, v) in data_vec.iter_mut().enumerate() {
            if let FillEntry::DataBlock(_, floats, flags, buf) = v {
                let count = match &layout[i].ty {
                    UniformBindingType::DataBlock { members } => members.len(),
                    _ => panic!()
                };
                assert_eq!(*flags, (1 << count) - 1, "DataBlock not filled");
                *buf = Some(device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(floats),
                        usage: wgpu::BufferUsage::UNIFORM
                    }
                ))
            }
        }

        let mut bindings: Vec<wgpu::BindGroupEntry> = vec![];
        for x in &data_vec {
            match x {
                FillEntry::Property(binding, p) => {
                    bindings.push(wgpu::BindGroupEntry {
                        binding: *binding,
                        resource: match p.expect("Property not assigned") {
                            MatProperty::Sampler(smp) =>
                                wgpu::BindingResource::Sampler(res_mgr.get(smp)),
                            MatProperty::Texture(tex) => {
                                wgpu::BindingResource::TextureView(&res_mgr.get(tex).default_view)
                            },
                            MatProperty::TextureSampler(tex) => {
                                wgpu::BindingResource::Sampler(&res_mgr.get(tex).sampler)
                            }
                            _ => panic!()
                        }
                    });
                },
                FillEntry::DataBlock(binding, floats, _, buf) => {
                    let slice = buf.as_ref().unwrap().slice(0..((floats.len() * std::mem::size_of::<f32>()) as wgpu::BufferAddress));
                    bindings.push(wgpu::BindGroupEntry {
                        binding: *binding,
                        resource: wgpu::BindingResource::Buffer(slice)
                    });
                }
            }
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &program.bind_group_layout,
            label: Some("Some material"),
            entries: &bindings
        });

        drop(bindings);
        bind_group
    }

}

impl Material {

    pub fn create(
        res_mgr: &ResManager,
        wgpu_states: &WgpuState,
        program: ResourceRef<ShaderProgram>,
        properties: HashMap<String, MatProperty>) -> Self {
        let shader_program = res_mgr.get(&program);
        let bind_group = Self::create_bind_group(res_mgr, &shader_program, &wgpu_states.device, &properties);
        Self {
            program,
            properties,
            bind_group,
            dirty: false
        }
    }

}

mod internal {
    use super::*;
    use futures::executor::*;
    use futures::task::{LocalSpawnExt};

    pub struct SysRenderPrepare {}

    pub struct SysRenderTeardown {
        local_pool: LocalPool
    }

    impl SysRenderTeardown {
        pub fn new() -> Self {
            Self {
                local_pool: LocalPool::new()
            }
        }
    }

    impl<'a> System<'a> for SysRenderPrepare {
        type SystemData = (ReadExpect<'a, WindowInfo>, ReadExpect<'a, WgpuState>,
                           Entities<'a>, ReadStorage<'a, Camera>, ReadStorage<'a, Transform>);

        fn run(&mut self, (window_info, wgpu_state, entities, cameras, transforms): Self::SystemData) {
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
                                    attachment: &wgpu_state.frame_texture.as_ref().unwrap().output.view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(color.into()),
                                        store: true
                                    }
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
                camera_infos: result_vec,
                // FIXME: Currently allocates every frame, but actually this can be reused?
                staging_belt: wgpu::util::StagingBelt::new(1024)
            })
        }
    }

    impl<'a> System<'a> for SysRenderTeardown {
        type SystemData = ReadExpect<'a, WgpuState>;

        fn run(&mut self, wgpu_state: Self::SystemData) {
            let mut result = clear_render_data();
            result.staging_belt.finish();
            wgpu_state.queue.submit(
                result.camera_infos
                    .into_iter()
                    .map(|x| x.encoder.finish())
                    .collect::<Vec<_>>());

            let spawner = self.local_pool.spawner();
            spawner.spawn_local(result.staging_belt.recall())
                .expect("Recall staging belt");
        }
    }
}

pub struct GraphicsModule;

impl Module for GraphicsModule {
    fn init(&self, init_data: &mut crate::InitContext) {
        use crate::InsertInfo;
        {
            init_data.dispatch_thread_local(
                InsertInfo::new(DEP_CAM_DRAW_SETUP)
                    .before(&[DEP_CAM_DRAW_TEARDOWN])
                    .order(100),
                move |_, i| { i.insert_thread_local(internal::SysRenderPrepare {}) },
            );
        }
        init_data.dispatch_thread_local(
            InsertInfo::new(DEP_CAM_DRAW_TEARDOWN).after(&[DEP_CAM_DRAW_SETUP]),
            |_, i| i.insert_thread_local(internal::SysRenderTeardown::new()),
        );
    }

    fn start(&self, ctx: &mut crate::StartContext) {
        if let Some(mut res) = ctx.world.try_fetch_mut::<AssetInspectorResources>() {
            res.add_factory(".tex.json", SerializeConfigInspectorFactory::<TextureConfig>::new());
            res.add_factory(".shader.json", SerializeConfigInspectorFactory::<ShaderConfig>::new());
        }
    }
}
