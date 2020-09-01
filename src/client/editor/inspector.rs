use specs::prelude::*;
use imgui::*;
use std::borrow::Borrow;
use std::path::PathBuf;

pub enum InspectEntry {
    Asset(PathBuf)
}

pub trait InspectHandler {

    fn accepts(&self, entry: &InspectEntry) -> bool;
    fn display(&self, ui: &Ui, entry: &InspectEntry);

}

type InspectHandlerBox = Box<dyn InspectHandler + Send + Sync>;

pub struct InspectorRuntimeData {
    pub handlers: Vec<InspectHandlerBox>,
    pub current: Option<InspectEntry>,
    pub pinned: Vec<InspectEntry>
}

impl InspectorRuntimeData {

    pub fn new() -> Self {
        Self {
            handlers: vec![],
            current: None,
            pinned: vec![]
        }
    }

}

pub(crate) struct InspectorSystem;

fn _show_inspector(ui: &Ui, handlers: &Vec<InspectHandlerBox>, item: &InspectEntry) -> bool {
    Window::new(im_str!("Inspector"))
        .build(ui, || {
            for handler in handlers {
                if handler.accepts(item) {
                    handler.display(ui, item);
                    return
                }
            }
        });

    true
}

impl<'a> System<'a> for InspectorSystem {
    type SystemData = (ReadExpect<'a, InspectorRuntimeData>);

    fn run(&mut self, data: Self::SystemData) {
        super::with_frame(|ui| {
            if let Some(current) = &data.current {
                _show_inspector(ui, &data.handlers, current);
            }
        });
    }
}