use specs::prelude::*;
use std::cell::RefCell;
use glium::{Frame, Display, Surface};

use crate::util::Color;
use crate::math::{Mat4, Vec3, Deg};

pub const DEP_RENDER_SETUP: &str = "render_setup";
pub const DEP_RENDER_TEARDOWN: &str = "render_teardown";

pub struct CamRenderData {
    pub wvp_matrix: Mat4,
    pub world_pos: Vec3,
}

pub struct FrameRenderData {
    pub frame: Frame,
    pub camera_infos: Vec<CamRenderData>,
}

thread_local!(
    static FRAME_RENDER_DATA: RefCell<Option<FrameRenderData>> = RefCell::new(None);
);

/// Acquire the render data reference in the closure,
/// and (presumably) do the rendering.
pub fn with_render_data<F>(mut f: F)
    where
        F: FnMut(&mut FrameRenderData),
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
    type SystemData = (ReadStorage<'a, Camera>, ReadStorage<'a, Transform>);

    fn run(&mut self, (cameras, transforms): Self::SystemData) {
        let mut frame = self.display.draw();
        let cam_infos = {
            let mut res: Vec<CamRenderData> = vec![];
            for (cam, trans) in (&cameras, &transforms).join() {
                // Calculate wvp matrix
                let aspect: f32 = 1.6;

                let projection = match cam.projection {
                    CameraProjection::Perspective { fov, z_near, z_far } => {
                        crate::math::perspective(crate::math::deg(fov), aspect, z_near, z_far)
                    }
                    CameraProjection::Orthographic { size, z_near, z_far } => {
                        let half_size = size / 2.;

                        crate::math::ortho(-aspect * half_size, aspect * half_size,
                            -half_size, half_size,
                            z_near, z_far)
                    }
                };
                // let perspective: Mat4 = crate::math::cgmath::perspective()
                //     .as_matrix()
                //     .clone();
                let rot = Mat4::from(trans.get_rotation());

                //            rot[(3, 3)] = 1.0;
                // !!!! TODO
                let world_view: Mat4 = rot;
                // let world_view: Mat4 = rot * crate::math::cgmath::Transform3::(&-trans.pos);

                //            let mut wvp_matrix = Mat4::identity();
                //            wvp_matrix[(0, 3)] = transform.pos[0];
                //            wvp_matrix[(1, 3)] = transform.pos[1];
                //            wvp_matrix[(2, 3)] = transform.pos[2];

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

use std::rc::Rc;
use crate::ecs::Transform;
use crate::Module;

pub struct GraphicsModule {
    display: Rc<Display>,
}

impl GraphicsModule {
    pub fn new(display: Rc<Display>) -> Self {
        GraphicsModule { display }
    }
}

impl Module for GraphicsModule {
    fn init(&self, init_data: &mut crate::InitData) {
        use crate::InsertInfo;
        {
            let display_clone = self.display.clone();
            init_data.dispatch_thread_local(
                InsertInfo::new("render_setup")
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
            InsertInfo::new("render_teardown").after(&[DEP_RENDER_SETUP]),
            |f| f.insert_thread_local(SysRenderTeardown {}),
        );
    }

    // fn start(&self, _start_data: &mut crate::StartData) {}
}
