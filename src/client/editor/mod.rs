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
use imgui_wgpu::{Renderer, RendererConfig};
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

pub struct EditorUIResources {
    pub show_ui: bool,
    pub demo_window_opened: bool,
    pub all_toggle_views: Vec<ToggleViewEntry>,
    pub renderer: Renderer,
    // TODO: Serialize settings
    pub all_opened_views: HashSet<String>,
}

impl EditorUIResources {

    pub fn new(context: &mut imgui::Context, ws: &WgpuState) -> Self {
        let mut renderer_config = RendererConfig::new_srgb();
        renderer_config.texture_format = ws.sc_desc.format;

        let renderer = Renderer::new(context, &ws.device, &ws.queue, renderer_config);
        Self {
            show_ui: true,
            demo_window_opened: false,
            all_opened_views: HashSet::new(),
            all_toggle_views: vec![],
            renderer
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
}

impl EditorUISetupSystem {
    fn new(mut imgui: imgui::Context, window: Rc<Window>) -> Self {
        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &*window, HiDpiMode::Default);

        Self {
            imgui,
            platform,
            window,
        }
    }
}

impl<'a> System<'a> for EditorUISetupSystem {
    type SystemData = (ReadExpect<'a, Time>,
                       ReadExpect<'a, WindowInfo>, WriteExpect<'a, EditorUIResources>);

    fn run(&mut self, (time, data, mut ui_res_write): Self::SystemData) {
        let ui_res = &mut *ui_res_write;
        let data = &data;

        let delta_time = std::time::Duration::from_secs_f32(time.get_delta_time());

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

        self.imgui.io_mut().update_delta_time(delta_time);

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
}

impl EditorUITeardownSystem {
    pub fn new() -> EditorUITeardownSystem {
        Self {}
    }
}

impl<'a> System<'a> for EditorUITeardownSystem {
    type SystemData = (ReadExpect<'a, WgpuState>, WriteExpect<'a, EditorUIResources>);

    fn run(&mut self, (wgpu_state, mut ui_res): Self::SystemData) {
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

                ui_res.renderer
                    .render(&draw_data, &wgpu_state.queue, &wgpu_state.device, &mut rpass)
                    .expect("Rendering imgui failed");

                drop(rpass);
                wgpu_state.queue.submit(Some(encoder.finish()));
            }
            _ => (),
        }
    }
}

#[derive(Copy, Clone)]
pub enum EditState {
    Clean,
    Dirty(Instant)
}

impl EditState {

    pub fn should_save(&mut self) -> bool {
        const THRESHOLD: f32 = 1.0;
        if let EditState::Dirty(instant) = *self {
            if (Instant::now() - instant).as_secs_f32() > THRESHOLD {
                return true;
            }
        }
        return false;
    }

    pub fn mark_clean(&mut self) {
        *self = EditState::Clean;
    }

    pub fn mark_dirty(&mut self) {
        *self = EditState::Dirty(Instant::now());
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

        let mut ui_res = EditorUIResources::new(&mut ctx, &*init_ctx.init_data.world.read_resource());
        ui_res.push_view_toggle(DEMO_WINDOW_TOGGLE, "IMGUI Demo");

        {
            let insert_info = InsertInfo::new(DEP_IMGUI_TEARDOWN).after(&[DEP_IMGUI_SETUP]);
            let sys = EditorUITeardownSystem::new();
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
            ui_res.push_view_toggle(asset_editor::VIEW_TOGGLE_ID, "Assets");
            ui_res.all_opened_views.insert(asset_editor::VIEW_TOGGLE_ID.to_string());

            init_ctx.init_data.world.insert(asset_editor::AssetEditorEvents::new());
            init_ctx.init_data.world.insert(asset_editor::AssetEditorResource::new(&asset_path));
            init_ctx.group_thread_local.dispatch(
                InsertInfo::default().after(&[DEP_IMGUI_SETUP]).before(&[DEP_IMGUI_TEARDOWN]),
                |_, i| i.insert_thread_local(asset_editor::AssetEditorSystem {})
            );

            init_ctx.init_data.world.insert(asset_editor::AssetInspectorResources::new());
            init_ctx.group_thread_local.dispatch(
                InsertInfo::default().before(&[DEP_IMGUI_TEARDOWN]).after(&[DEP_IMGUI_SETUP]),
                |_, i| i.insert_thread_local(asset_editor::InspectorSystem)
            );
        }


        init_ctx.init_data.world.insert(ui_res);
    }

    fn name(&self) -> &'static str { MODULE_NAME }
}

