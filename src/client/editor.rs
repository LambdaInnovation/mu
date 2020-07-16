use crate::{Module, InitData, InsertInfo};
use specs::{System, ReadExpect};
use crate::client::WindowInfo;
use crate::client::graphics;
use imgui::{Ui, FontSource, FontConfig};
use imgui_winit_support::{WinitPlatform, HiDpiMode};
use glium::Display;
use std::time::Instant;
use std::rc::Rc;
use imgui_glium_renderer::Renderer;

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
    display: Rc<Display>,
    last_frame: Option<Instant>
}

impl EditorUISetupSystem {
    fn new(mut imgui: imgui::Context, display: Rc<Display>) -> Self {
        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), display.gl_window().window(), HiDpiMode::Default);

        Self {
            display,
            imgui,
            platform,
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
                self.display.gl_window().window(),
                evt
            );
        }

        self.platform
            .prepare_frame(self.imgui.io_mut(), self.display.gl_window().window())
            .expect("Failed to prepare frame!");

        let last_frame = match self.last_frame {
            Some(instant) => instant,
            None => Instant::now()
        };
        self.last_frame = Some(self.imgui.io_mut().update_delta_time(last_frame));

        let ui = self.imgui.frame();

        let mut enable = false;
        ui.show_demo_window(&mut enable);

        self.platform.prepare_render(&ui, self.display.gl_window().window());

        unsafe { FRAME = Some(std::mem::transmute::<Ui<'_>, Ui<'static>>(ui)) };
    }
}

struct EditorUITeardownSystem {
    renderer: Renderer,
}

impl EditorUITeardownSystem {
    pub fn new(context: &mut imgui::Context, display: &Display) -> EditorUITeardownSystem {
        let renderer = Renderer::init(context, display).expect("Failed to initialize renderer");
        Self { renderer }
    }
}

impl<'a> System<'a> for EditorUITeardownSystem {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
        let ui_opt = unsafe { FRAME.take() };
        match ui_opt {
            Some(ui) => {
                let draw_data = ui.render();

                graphics::with_render_data(|data| {
                    self.renderer
                        .render(&mut data.frame, draw_data)
                        .expect("Rendering imgui failed");
                });
            }
            _ => (),
        }
    }
}

pub struct EditorModule;

impl Module for EditorModule {
    fn init(&self, init_data: &mut InitData) {
        let mut ctx = imgui::Context::create();
        ctx.set_ini_filename(None);
        let hidpi_factor = init_data.display.gl_window().window().scale_factor();
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
            let insert_info = InsertInfo::new(DEP_TEARDOWN)
                .after(&[DEP_SETUP])
                .before(&[graphics::DEP_RENDER_TEARDOWN])
                .order(graphics::render_order::DEBUG_UI);
            let sys = EditorUITeardownSystem::new(&mut ctx, &init_data.display);
            init_data.group_thread_local.dispatch(
                insert_info,
                move |f| f.insert_thread_local(sys)
            );
        }

        {
            let insert_info = InsertInfo::new(DEP_SETUP)
                .after(&[graphics::DEP_RENDER_SETUP])
                .order(graphics::render_order::DEBUG_UI);
            let sys = EditorUISetupSystem::new(ctx, init_data.display.clone());
            init_data.group_thread_local.dispatch(
                insert_info,
                |f| f.insert_thread_local(sys));
        }
    }
}
