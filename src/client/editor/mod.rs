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
use std::collections::HashSet;

pub mod inspect;
pub mod asset_editor;

pub const DEP_IMGUI_SETUP: &str = "editor_setup";
pub const DEP_IMGUI_TEARDOWN: &str = "editor_teardown";
pub const MODULE_NAME: &str = "editor";

const DEMO_WINDOW_TOGGLE: &str = "imgui_demo";
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

pub struct ToggleViewEntry {
    pub id: String,
    pub display_name: String,
}

pub(crate) struct EditorUIResources {
    pub show_ui: bool,
    pub demo_window_opened: bool,
    pub all_toggle_views: Vec<ToggleViewEntry>,
    // TODO: Serialize settings
    pub all_opened_views: HashSet<String>,
}

impl EditorUIResources {

    pub fn new() -> Self {
        Self {
            show_ui: true,
            demo_window_opened: false,
            all_opened_views: HashSet::new(),
            all_toggle_views: vec![],
        }
    }

    pub fn push_view_toggle(&mut self, id: &str, display_name: &str) {
        self.all_toggle_views.push(ToggleViewEntry {
            id: id.to_string(),
            display_name: display_name.to_string()
        });
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
    type SystemData = (ReadExpect<'a, WindowInfo>, WriteExpect<'a, EditorUIResources>);

    fn run(&mut self, (data, mut ui_res_write): Self::SystemData) {
        let ui_res = &mut *ui_res_write;
        let data = &data;

        if !ui_res.show_ui {
            return
        }

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
        ui.main_menu_bar(|| {
            ui.menu(im_str!("View"), true, || {
                // let (all_toggle_views, all_opened_views) = ui_res.borrow_view_states();
                for view_entry in &ui_res.all_toggle_views {
                    let last_enabled = ui_res.all_opened_views.contains(&view_entry.id);
                    if MenuItem::new(&im_str!("{}", view_entry.display_name))
                        .selected(last_enabled)
                        .build(&ui) {
                        let enabled = !last_enabled;
                        if enabled {
                            ui_res.all_opened_views.insert(view_entry.id.clone());
                        } else {
                            ui_res.all_opened_views.remove(&view_entry.id);
                        }
                    }
                }
            });
        });

        if ui_res.all_opened_views.contains(DEMO_WINDOW_TOGGLE) {
            ui.show_demo_window(&mut ui_res.demo_window_opened);
        }

        self.platform.prepare_render(&ui, &*self.window);

        unsafe { FRAME = Some(std::mem::transmute::<Ui<'_>, Ui<'static>>(ui)) };
    }
}

struct EditorUITeardownSystem {
    renderer: Renderer,
}

impl EditorUITeardownSystem {
    pub fn new(context: &mut imgui::Context, wgpu_state: &WgpuState) -> EditorUITeardownSystem {
        let renderer =
            Renderer::new(context, &wgpu_state.device, &wgpu_state.queue, wgpu_state.sc_desc.format);
        Self {
            renderer,
        }
    }
}

impl<'a> System<'a> for EditorUITeardownSystem {
    type SystemData = ReadExpect<'a, WgpuState>;

    fn run(&mut self, wgpu_state: Self::SystemData) {
        let ui_opt = unsafe { FRAME.take() };
        match ui_opt {
            Some(ui) => {
                let draw_data = ui.render();
                let mut encoder = wgpu_state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Mu editor")
                });

                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

                self.renderer
                    .render(&draw_data, &wgpu_state.queue, &wgpu_state.device, &mut rpass)
                    .expect("Rendering imgui failed");

                drop(rpass);
                wgpu_state.queue.submit(Some(encoder.finish()));
            }
            _ => (),
        }
    }
}

pub struct EditorModule {
    pub asset_path: Option<String>, // Path to /asset folder of a project. If not set, editor-only features will be disabled.
}

impl Module for EditorModule {
    fn init(&self, init_ctx: &mut InitContext) {
        let mut ctx = imgui::Context::create();
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

        let mut ui_res = EditorUIResources::new();
        ui_res.push_view_toggle(DEMO_WINDOW_TOGGLE, "IMGUI Demo");
        ui_res.push_view_toggle(asset_editor::VIEW_TOGGLE_ID, "Assets");
        ui_res.all_opened_views.insert(asset_editor::VIEW_TOGGLE_ID.to_string());
        init_ctx.init_data.world.insert(ui_res);

        {
            let insert_info = InsertInfo::new(DEP_IMGUI_TEARDOWN).after(&[DEP_IMGUI_SETUP]);
            let sys = EditorUITeardownSystem::new(&mut ctx,
                                                  &*init_ctx.init_data.world.read_resource());
            init_ctx.group_thread_local.dispatch(
                insert_info,
                move |_, i| i.insert_thread_local(sys)
            );
        }

        {
            let insert_info = InsertInfo::new(DEP_IMGUI_SETUP)
                .after(&[graphics::DEP_CAM_DRAW_TEARDOWN]);
            let sys = EditorUISetupSystem::new(ctx, init_ctx.init_data.window.clone());
            init_ctx.group_thread_local.dispatch(
                insert_info,
                |_, f| f.insert_thread_local(sys));
        }

        if let Some(asset_path) = &self.asset_path {
            init_ctx.init_data.world.insert(asset_editor::AssetEditorResource::new(&asset_path));
            init_ctx.group_thread_local.dispatch(
                InsertInfo::default().after(&[DEP_IMGUI_SETUP]).before(&[DEP_IMGUI_TEARDOWN]),
                |_, i| i.insert_thread_local(asset_editor::AssetEditorSystem {})
            );
        }

        init_ctx.init_data.world.insert(asset_editor::AssetInspectorResources::new());
        init_ctx.group_thread_local.dispatch(
            InsertInfo::default().before(&[DEP_IMGUI_TEARDOWN]).after(&[DEP_IMGUI_SETUP]),
            |_, i| i.insert_thread_local(asset_editor::InspectorSystem)
        );
    }

    fn name(&self) -> &'static str { MODULE_NAME }
}

