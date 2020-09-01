use imgui::*;
use super::{with_frame, DEP_IMGUI_TEARDOWN, DEP_IMGUI_SETUP};
use specs::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::fs::DirEntry;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use crate::client::editor::inspector::{InspectorRuntimeData, InspectEntry};

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

struct AssetGuiContext<'a> {
    editor_info: &'a mut AssetEditorInfo,
    inspector_info_write: &'a mut InspectorRuntimeData
}

pub(crate) struct AssetEditorInfo {
    pub base_path: PathBuf,
    selected: u64,
    fs_path_cache: HashMap<PathBuf, Vec<CachedDirEntry>>
}

impl AssetEditorInfo {

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

fn _walk_path(ctx: &mut AssetGuiContext, ui: &Ui, path: &PathBuf) {
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
                            ctx.inspector_info_write.current = Some(InspectEntry::Asset(e.path))
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
    type SystemData = (WriteExpect<'a, AssetEditorInfo>, WriteExpect<'a, InspectorRuntimeData>);

    fn run(&mut self, (mut info, mut inspector_data): Self::SystemData) {
        with_frame(|ui| {
            Window::new(im_str!("Assets")).build(ui, || {
                let base_path = info.base_path.clone();
                let mut ctx = AssetGuiContext {
                    editor_info: &mut *info,
                    inspector_info_write: &mut *inspector_data
                };
                _walk_path(&mut ctx, ui, &base_path);
            });
        });
    }
}