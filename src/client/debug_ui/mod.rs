use super::window::CursorGrab;
use crate::input;
use crate::input::RawInputData;
use crate::InsertInfo;
use crate::client::{Event, EventLoop};
use glium::*;
use imgui::*;
use imgui_glium_renderer::Renderer;
use imgui_winit_support::*;
use specs::shrev::EventChannel;
use specs::prelude::*;
use std::rc::Rc;
use std::time::*;
use glutin::window;

pub const DEP_SETUP: &str = "imgui_setup";
pub const DEP_TEARDOWN: &str = "imgui_teardown";

// mod perf;

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

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugUIActive {
    pub active: bool,
}

struct SysDebugUI {
    event_reader: Option<ReaderId<Event>>,
}

impl SysDebugUI {
    pub fn new() -> Self {
        SysDebugUI { event_reader: None }
    }
}

impl<'a> System<'a> for SysDebugUI {
    type SystemData = (
        ReadExpect<'a, RawInputData>,
        Write<'a, DebugUIActive>,
        Write<'a, CursorGrab>,
    );

    fn run(&mut self, (raw_input, mut debug_ui_active, mut cursor_grab): Self::SystemData) {
        if raw_input.get_key_state(input::VirtualKeyCode::Grave) == input::ButtonState::Down {
            debug_ui_active.active = !debug_ui_active.active;
        }

        // ! This is the only place that we grab cursor for now
        cursor_grab.grabbed = !debug_ui_active.active;
    }

    fn setup(&mut self, res: &mut World) {
        res.insert(DebugUIActive { active: false });
        Self::SystemData::setup(res);
        self.event_reader = Some(res.fetch_mut::<EventChannel<Event>>().register_reader());
    }
}

struct SysSetup {
    display: Rc<Display>,
    event_reader: Option<ReaderId<Event>>,
    imgui: imgui::Context,
    platform: WinitPlatform,
    last_frame: Option<Instant>,
    demo_enable: bool,
}

impl SysSetup {
    pub fn new(mut imgui: imgui::Context, display: Rc<Display>) -> Self {
        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(
            imgui.io_mut(),
            display.gl_window().window(),
            HiDpiMode::Default,
        );

        Self {
            display: display.clone(),
            event_reader: None,
            imgui,
            platform,
            last_frame: None,
            demo_enable: true,
        }
    }
}

impl<'a> System<'a> for SysSetup {
    type SystemData = (Read<'a, EventChannel<Event>>, ReadExpect<'a, DebugUIActive>);

    fn run(&mut self, (events, debug_ui_active): Self::SystemData) {
        let active = debug_ui_active.active;
        if active {
            for event in events.read(&mut self.event_reader.as_mut().unwrap()) {
                self.platform.handle_event(
                    self.imgui.io_mut(),
                    self.display.gl_window().window(),
                    event,
                );
            }
            self.platform
                .prepare_frame(self.imgui.io_mut(), self.display.gl_window().window())
                .expect("Failed to prepare frame");

            let last_frame = match self.last_frame {
                Some(instant) => instant,
                None => Instant::now(),
            };
            self.last_frame = Some(self.imgui.io_mut().update_delta_time(last_frame));

            let ui = self.imgui.frame();

            ui.show_demo_window(&mut self.demo_enable); // Show demo window

            self.platform
                .prepare_render(&ui, self.display.gl_window().window());
            unsafe { FRAME = Some(std::mem::transmute::<Ui<'_>, Ui<'static>>(ui)) };
        }
    }

    fn setup(&mut self, res: &mut World) {
        Self::SystemData::setup(res);
        self.event_reader = Some(res.fetch_mut::<EventChannel<Event>>().register_reader());
    }
}

struct SysTeardown {
    renderer: Renderer,
}

impl SysTeardown {
    pub fn new(context: &mut imgui::Context, display: &Display) -> SysTeardown {
        let renderer = Renderer::init(context, display).expect("Failed to initialize renderer");
        Self { renderer }
    }
}

impl<'a> System<'a> for SysTeardown {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
        use super::graph;

        let ui_opt = unsafe { FRAME.take() };
        match ui_opt {
            Some(ui) => {
                let draw_data = ui.render();

                graph::with_render_data(|data| {
                    self.renderer
                        .render(&mut data.frame, draw_data)
                        .expect("Rendering imgui failed");
                });
            }
            _ => (),
        }
    }
}

use crate::game_loop::Module;

pub struct DebugUIModule {
    display: Rc<Display>,
}

impl DebugUIModule {
    pub fn new(display: Rc<Display>) -> Self {
        DebugUIModule { display }
    }
}

impl Module for DebugUIModule {
    fn build(&mut self, init_data: &mut crate::InitData) {
        use super::graph;
        init_data.dispatch(InsertInfo::new("debug_ui").after(&["input"]), |f| {
            f.insert(SysDebugUI::new())
        });

        let mut ctx = imgui::Context::create();
        ctx.set_ini_filename(None);
        // let hidpi_factor = self.display.gl_window().window().get_hidpi_factor();
        let hidpi_factor = 1.0;
        let font_size = (13.0 * hidpi_factor) as f32;

        ctx.fonts().add_font(&[
            FontSource::DefaultFontData {
                config: Some(FontConfig {
                    size_pixels: font_size,
                    ..FontConfig::default()
                }),
            },
            // FontSource::TtfData {
            //     include_bytes!("../../assets/resources/mplus-1p-regular.ttf"),
            //     size_pixels: font_size,
            //     config: Some(FontConfig {
            //         rasterizer_multiply: 1.75,
            //         glyph_ranges: FontGlyphRanges::japanese(),
            //         ..FontConfig::default()
            //     })
            // }
        ]);

        ctx.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        {
            let sys_teardown = SysTeardown::new(&mut ctx, &*self.display);
            init_data.dispatch_thread_local(
                InsertInfo::new(DEP_TEARDOWN)
                    .after(&[DEP_SETUP])
                    .before(&[graph::DEP_RENDER_TEARDOWN])
                    .order(graph::order::DEBUG_UI),
                move |f| f.insert_thread_local(sys_teardown),
            );
        }

        let display = self.display.clone();
        init_data.dispatch_thread_local(
            InsertInfo::new(DEP_SETUP)
                .after(&[graph::DEP_RENDER_SETUP])
                .order(graph::order::DEBUG_UI),
            move |f| f.insert_thread_local(SysSetup::new(ctx, display)),
        );
    }

    // fn get_submodules(&mut self) -> Vec<Box<Module>> {
    //     vec![Box::new(perf::PerfModule::new())]
    // }
}
