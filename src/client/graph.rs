use crate::asset;
use crate::math::*;

use glium::*;
use specs;
use specs::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::util::{Color, Transform};
use glium::program::ProgramCreationInput;

pub const DEP_RENDER_SETUP: &str = "render_setup";
pub const DEP_RENDER_TEARDOWN: &str = "render_teardown";

/// Default (universal) render orders
#[allow(dead_code)]
pub mod order {
    pub const OPAQUE: i32 = 0;
    pub const TRANSPARENT: i32 = 1000;
    pub const UI: i32 = 10000;
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

pub enum CameraPerspective {
    Orthogonal {
        size: Float,
    },
    #[allow(dead_code)]
    Projection {
        size: Float,
        fov: Float,
        z_near: Float,
        z_far: Float,
    },
}

pub struct Camera {
    pub pos: Vec3,
    pub rot: Quaternion,
    pub persp: CameraPerspective,
    // Clear color
    pub clear_color: Option<Color>,
    pub clear_depth: bool,
}

impl Camera {
    pub fn new() -> Camera {
        Camera {
            pos: Vec3::zero(),
            rot: Quaternion::identity(),
            persp: CameraPerspective::Orthogonal { size: 1.0 },
            clear_color: Some(Color::rgb(0.0, 0.0, 0.0)),
            clear_depth: true,
        }
    }
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
                let perspective: Mat4 = geometry::Perspective3::new(1.6, 1.04667, 0.001, 1000.0)
                    .as_matrix()
                    .clone();
                let rot = Mat4::from(trans.get_rotation().inverse());

                //            rot[(3, 3)] = 1.0;
                let world_view: Mat4 = rot * Mat4::new_translation(&-trans.pos);

                //            let mut wvp_matrix = Mat4::identity();
                //            wvp_matrix[(0, 3)] = transform.pos[0];
                //            wvp_matrix[(1, 3)] = transform.pos[1];
                //            wvp_matrix[(2, 3)] = transform.pos[2];

                let wvp_matrix = perspective * world_view;
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

use crate::game_loop::Module;

pub struct GraphModule {
    display: Rc<Display>,
}

impl GraphModule {
    pub fn new(display: Rc<Display>) -> Self {
        GraphModule { display }
    }
}

impl Module for GraphModule {
    fn build(&mut self, init_data: &mut crate::InitData) {
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

    fn on_start(&self, _start_data: &mut crate::StartData) {}
}
