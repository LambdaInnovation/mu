use imgui::*;
use super::{with_frame, DEP_IMGUI_TEARDOWN, DEP_IMGUI_SETUP};
use specs::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::fs::DirEntry;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::ffi::OsStr;

#[derive(Copy, Clone)]
enum DirEntryType {
    File, Directory
}

#[derive(Clone)]
struct CachedDirEntry {
    path: PathBuf,
    path_hash: u64,
    filename: ImString,
    ty: DirEntryType
}

pub(crate) struct AssetEditorResource {
    pub base_path: PathBuf,
    selected: u64,
    fs_path_cache: HashMap<PathBuf, Vec<CachedDirEntry>>
}

impl AssetEditorResource {

    pub fn new(path: &str) -> Self {
        Self {
            base_path: fs::canonicalize(PathBuf::from(path)).unwrap(),
            fs_path_cache: HashMap::new(),
            selected: 0
        }
    }

}

pub(crate) struct AssetEditorSystem {
}

struct AssetEditorPathRecurseContext<'a> {
    editor_info: &'a mut AssetEditorResource,
    inspector_info_write: &'a mut AssetInspectorResources
}

fn _walk_path(ctx: &mut AssetEditorPathRecurseContext, ui: &Ui, path: &PathBuf) {
    if !ctx.editor_info.fs_path_cache.contains_key(path) {
        let v = fs::read_dir(path)
            .map(|read_dir| read_dir
                .filter(|x| x.is_ok())
                .map(|x| x.unwrap())
                .map(|entry| {
                    let filetype = entry.file_type().unwrap();
                    (entry, filetype)
                })
                .filter(|(_, ft)| ft.is_dir() || ft.is_file())
                .map(|(entry, ft)| {
                    let ty = if ft.is_file() { DirEntryType::File } else { DirEntryType::Directory };
                    let hash = {
                        let mut hasher = DefaultHasher::new();
                        entry.path().hash(&mut hasher);
                        hasher.finish()
                    };
                    CachedDirEntry {
                        path: entry.path(),
                        path_hash: hash,
                        filename: ImString::new(entry.file_name().to_str().unwrap()),
                        ty
                    }
                })
                .collect::<Vec<_>>()
            )
            .unwrap_or(vec![]);
        ctx.editor_info.fs_path_cache.insert(path.clone(), v);
    }

    let entries = ctx.editor_info.fs_path_cache[path].clone(); // FIXME: clone is to bypass borrow check of `cache`, at the cost of performance
    for entry in entries.clone() {
        let is_selected = ctx.editor_info.selected == entry.path_hash;
        match entry.ty {
            DirEntryType::File => {
                // FIXME: Unnecessary clone
                let e = entry.clone();
                TreeNode::new(&entry.filename)
                    .label(&entry.filename)
                    .leaf(true)
                    .selected(is_selected)
                    .build(&ui, || {
                        if ui.is_item_clicked(MouseButton::Left) {
                            ctx.editor_info.selected = e.path_hash;
                            ctx.inspector_info_write.current =
                                Some(ctx.inspector_info_write.create_inspector(e.path.clone()));
                        }
                    });
            },
            DirEntryType::Directory => {
                TreeNode::new(&entry.filename)
                    .label(&entry.filename)
                    .selected(is_selected)
                    .build(&ui, || {
                        if ui.is_item_clicked(MouseButton::Left) {
                            ctx.editor_info.selected = entry.path_hash;
                        }
                        _walk_path(ctx, ui, &entry.path);
                    });
            }
        }
    }
}

impl<'a> System<'a> for AssetEditorSystem {
    type SystemData = (WriteExpect<'a, AssetEditorResource>, WriteExpect<'a, AssetInspectorResources>);

    fn run(&mut self, (mut info, mut inspector_data): Self::SystemData) {
        with_frame(|ui| {
            Window::new(im_str!("Assets")).build(ui, || {
                let base_path = info.base_path.clone();
                let mut ctx = AssetEditorPathRecurseContext {
                    editor_info: &mut *info,
                    inspector_info_write: &mut *inspector_data
                };
                _walk_path(&mut ctx, ui, &base_path);
            });
        });
    }
}

pub trait AssetInspectorFactory : Send + Sync {
    fn create(&self, path: PathBuf) -> Box<dyn AssetInspector>;
}

pub struct AssetInspectEntry {
    path: PathBuf,
    inspector: Box<dyn AssetInspector>
}

pub trait AssetInspector : Send + Sync {

    fn display(&mut self, ui: &Ui);

}

struct DefaultInspector {
}

impl AssetInspector for DefaultInspector {
    fn display(&mut self, ui: &Ui) {
        ui.text("Unsupported format");
    }
}

pub struct InspectorFactoryEntry {
    extension: String,
    factory: Box<dyn AssetInspectorFactory>
}

pub struct AssetInspectorResources {
    pub handlers: Vec<InspectorFactoryEntry>,
    pub current: Option<AssetInspectEntry>,
    pub pinned: Vec<AssetInspectEntry>
}

impl AssetInspectorResources {

    pub fn create_inspector(&self, path: PathBuf) -> AssetInspectEntry {
        for handler in &self.handlers {
            if path.ends_with(&handler.extension) {
                return AssetInspectEntry {
                    path: path.clone(),
                    inspector: handler.factory.create(path)
                }
            }
        }

        AssetInspectEntry {
            path,
            inspector: Box::new(DefaultInspector {})
        }
    }

}

impl AssetInspectorResources {

    pub fn new() -> Self {
        Self {
            handlers: vec![],
            current: None,
            pinned: vec![]
        }
    }

}

pub(crate) struct InspectorSystem;

fn _show_inspector(ui: &Ui, item: &mut AssetInspectEntry) -> bool {
    Window::new(im_str!("Inspector"))
        .build(ui, || {
            item.inspector.display(ui);
        });

    true
}

impl<'a> System<'a> for InspectorSystem {
    type SystemData = (WriteExpect<'a, AssetInspectorResources>);

    fn run(&mut self, mut data: Self::SystemData) {
        super::with_frame(|ui| {
            if let Some(current) = &mut data.current {
                _show_inspector(ui, current);
            }
        });
    }
}
