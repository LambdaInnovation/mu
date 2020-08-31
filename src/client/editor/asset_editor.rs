use imgui::*;
use super::{with_frame, DEP_IMGUI_TEARDOWN, DEP_IMGUI_SETUP};
use specs::prelude::*;

pub(crate) struct AssetEditorInfo {
    pub base_path: String
}

impl AssetEditorInfo {

    pub fn new(path: &str) -> Self {
        Self {
            base_path: path.to_string()
        }
    }

}

pub(crate) struct AssetEditorSystem {
}

impl<'a> System<'a> for AssetEditorSystem {
    type SystemData = ReadExpect<'a, AssetEditorInfo>;

    fn run(&mut self, info: Self::SystemData) {
        with_frame(|ui| {
            Window::new(im_str!("Assets")).build(ui, || {
                TreeNode::new(im_str!("File1.png")).leaf(true).build(ui, || {

                });
                TreeNode::new(im_str!("File2.png")).leaf(true).build(ui, || {

                });
            });
        });
    }
}