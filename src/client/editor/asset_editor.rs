use imgui::*;
use super::{with_frame, DEP_IMGUI_TEARDOWN, DEP_IMGUI_SETUP};
use specs::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::fs::DirEntry;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

#[derive(Copy, Clone)]
enum DirEntryType {
    File, Directory
}

#[derive(Clone)]
struct CachedDirEntry {
    path: PathBuf,
    filename: ImString,
    ty: DirEntryType
}

pub(crate) struct AssetEditorInfo {
    pub base_path: PathBuf,
    fs_path_cache: HashMap<PathBuf, Vec<CachedDirEntry>>
}

impl AssetEditorInfo {

    pub fn new(path: &str) -> Self {
        Self {
            base_path: fs::canonicalize(PathBuf::from(path)).unwrap(),
            fs_path_cache: HashMap::new()
        }
    }

}

pub(crate) struct AssetEditorSystem {
}

fn _walk_path(cache: &mut HashMap<PathBuf, Vec<CachedDirEntry>>, ui: &Ui, path: &PathBuf) {
    if !cache.contains_key(path) {
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
                    CachedDirEntry {
                        path: entry.path(),
                        filename: ImString::new(entry.file_name().to_str().unwrap()),
                        ty
                    }
                })
                .collect::<Vec<_>>()
            )
            .unwrap_or(vec![]);
        cache.insert(path.clone(), v);
    }

    let entries = cache[path].clone(); // FIXME: clone is to bypass borrow check of `cache`, at the cost of performance
    for entry in entries.clone() {
        match entry.ty {
            DirEntryType::File => {
                TreeNode::new(&entry.filename)
                    .label(&entry.filename)
                    .leaf(true)
                    .build(&ui, || {
                    });
            },
            DirEntryType::Directory => {
                TreeNode::new(&entry.filename)
                    .label(&entry.filename)
                    .build(&ui, || {
                        _walk_path(cache, ui, &entry.path);
                    });
            }
        }
    }
}

impl<'a> System<'a> for AssetEditorSystem {
    type SystemData = WriteExpect<'a, AssetEditorInfo>;

    fn run(&mut self, mut info: Self::SystemData) {
        with_frame(|ui| {
            Window::new(im_str!("Assets")).build(ui, || {
                let base_path = info.base_path.clone();
                let path_cache = &mut info.fs_path_cache;
                _walk_path(path_cache, ui, &base_path);
            });
        });
    }
}