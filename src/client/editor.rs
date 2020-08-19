use crate::*;
use specs::{System, ReadExpect};
use crate::client::WindowInfo;
use crate::client::graphics;
use winit;
use imgui::*;
use imgui_winit_support::{WinitPlatform, HiDpiMode};
use std::time::Instant;
use std::rc::Rc;
use winit::window::Window;
use imgui_wgpu::Renderer;

pub const DEP_SETUP: &str = "editor_setup";
pub const DEP_TEARDOWN: &str = "editor_teardown";

static mut FRAME: Option<Ui> = None;

pub fn with_frame<F>(f: F)
    where
        F: FnOnce(&Ui),
{
    unsafe {
        match &FRAME {
            Some(ref frame) => f(frame),
            //            _ => panic!("No frame available now")
            _ => (),
        }
    }
}

struct EditorUISetupSystem {
    platform: WinitPlatform,
    imgui: imgui::Context,
    window: Rc<Window>,
    last_frame: Option<Instant>
}

impl EditorUISetupSystem {
    fn new(mut imgui: imgui::Context, window: Rc<Window>) -> Self {
        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &*window, HiDpiMode::Default);

        Self {
            imgui,
            platform,
            window,
            last_frame: None,
        }
    }
}

impl<'a> System<'a> for EditorUISetupSystem {
    type SystemData = ReadExpect<'a, WindowInfo>;

    fn run(&mut self, data: Self::SystemData) {
        let data = &data;

        for evt in &data.frame_event_list {
            self.platform.handle_event(
                self.imgui.io_mut(),
                &*self.window,
                evt
            );
        }

        self.platform
            .prepare_frame(self.imgui.io_mut(), &*self.window)
            .expect("Failed to prepare frame!");

        let last_frame = match self.last_frame {
            Some(instant) => instant,
            None => Instant::now()
        };
        self.last_frame = Some(self.imgui.io_mut().update_delta_time(last_frame));

        let ui = self.imgui.frame();

        let mut enable = false;
        ui.show_demo_window(&mut enable);

        self.platform.prepare_render(&ui, &*self.window);

        unsafe { FRAME = Some(std::mem::transmute::<Ui<'_>, Ui<'static>>(ui)) };
    }
}

struct EditorUITeardownSystem {
    renderer: Renderer,
    wgpu_state: WgpuStateCell
}

impl EditorUITeardownSystem {
    pub fn new(context: &mut imgui::Context, wgpu_state_cell: WgpuStateCell) -> EditorUITeardownSystem {
        let renderer = {
            let wgpu_state = wgpu_state_cell.borrow();
            Renderer::new(context, &wgpu_state.device, &wgpu_state.queue, wgpu_state.sc_desc.format, None)
        };
        Self {
            renderer ,
            wgpu_state: wgpu_state_cell
        }
    }
}

impl<'a> System<'a> for EditorUITeardownSystem {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
        let ui_opt = unsafe { FRAME.take() };
        match ui_opt {
            Some(ui) => {
                let draw_data = ui.render();
                let wgpu_state = self.wgpu_state.borrow();

                let mut encoder = wgpu_state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Mu editor")
                });

                self.renderer
                    .render(&draw_data, &wgpu_state.device, &mut encoder,
                            &wgpu_state.frame_texture.as_ref().unwrap().view)
                    .expect("Rendering imgui failed");

                wgpu_state.queue.submit(&[encoder.finish()]);
            }
            _ => (),
        }
    }
}

pub struct EditorModule;

impl Module for EditorModule {
    fn init(&self, init_ctx: &mut InitContext) {
        let mut ctx = imgui::Context::create();
        ctx.set_ini_filename(None);
        let hidpi_factor = init_ctx.init_data.window.scale_factor();
        let font_size = (13.0 * hidpi_factor) as f32;

        ctx.fonts().add_font(&[
            FontSource::DefaultFontData {
                config: Some(FontConfig {
                    size_pixels: font_size,
                    ..FontConfig::default()
                }),
            },
        ]);

        ctx.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        {
            let insert_info = InsertInfo::new(DEP_TEARDOWN).after(&[DEP_SETUP]);
            let sys = EditorUITeardownSystem::new(&mut ctx,
                                                  init_ctx.init_data.wgpu_state.clone());
            init_ctx.group_thread_local.dispatch(
                insert_info,
                move |_, f| f.insert_thread_local(sys)
            );
        }

        {
            let insert_info = InsertInfo::new(DEP_SETUP)
                .after(&[graphics::DEP_CAM_DRAW_TEARDOWN]);
            let sys = EditorUISetupSystem::new(ctx, init_ctx.init_data.window.clone());
            init_ctx.group_thread_local.dispatch(
                insert_info,
                |_, f| f.insert_thread_local(sys));
        }
    }
}
