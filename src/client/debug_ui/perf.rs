//! Performance module

use crate::game_loop::{FpsInfo, Module};
use crate::*;
use specs::prelude::*;

pub struct PerfModule {}

impl PerfModule {
    pub fn new() -> Self {
        Self {}
    }
}

impl Module for PerfModule {
    fn build(&mut self, init_data: &mut InitData) {
        {
            init_data.dispatch_thread_local(
                InsertInfo::new("perf_debug_ui")
                    .after(&[super::DEP_SETUP])
                    .before(&[super::DEP_TEARDOWN]),
                |f| f.insert_thread_local(SysPerfGui {}),
            )
        }
    }
}

struct SysPerfGui {}

impl<'a> System<'a> for SysPerfGui {
    type SystemData = (
        Option<Read<'a, FpsInfo>>,
        Option<Read<'a, voxel::VoxelProfileData>>,
    );

    fn run(&mut self, (fps_info, profile_info): Self::SystemData) {
        let fps = fps_info.map(|x| (&x).fps).unwrap_or(0.0);
        super::with_frame(|ui| {
            ui.window(im_str!("Performance")).build(|| {
                ui.text(im_str!("FPS: {}", fps));
                if let Some(info) = &profile_info {
                    if ui.collapsing_header(im_str!("Voxel")).build() {
                        ui.text(im_str!("leaves: {}", info.render_leaves));
                        ui.text(im_str!("parents: {}", info.parents));
                        ui.text(im_str!("time self: {} ns", info.time_self.as_nanos()));
                        ui.text(im_str!("time all : {} ns", info.time_total.as_nanos()));
                        ui.text(im_str!("time s1  : {} ns", info.time_s1.as_nanos()));
                        ui.text(im_str!("culled count : {}", info.culled_count));
                    }
                }
            });
        });
    }
}
