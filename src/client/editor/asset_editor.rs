use imgui::*;
use super::{with_frame};
use specs::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use crate::util::Color;
use crate::client::editor::EditorUIResources;
use imgui_inspect::*;
use serde::*;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use crate::asset::*;
use std::time::Instant;
use std::any::Any;

#[derive(Copy, Clone)]
enum DirEntryType {
    File, Directory
}

#[derive(Clone)]
struct CachedDirEntry {
    path: PathBuf,
    relative_path: PathBuf,
    path_hash: u64,
    filename: ImString,
    ty: DirEntryType
}

pub(crate) struct AssetEditorResource {
    pub base_path: PathBuf,
    selected: u64,
    fs_path_cache: HashMap<PathBuf, Vec<CachedDirEntry>>
}

/// used to communicate between unrelated systems.
pub enum AssetEditorEvent {
    Custom(Box<dyn Send+Sync+Any>)
}

pub type AssetEditorEvents = Vec<AssetEditorEvent>;

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
                    let relative_path = PathBuf::from(entry.path().strip_prefix(&ctx.editor_info.base_path).unwrap());
                    CachedDirEntry {
                        path: entry.path(),
                        path_hash: hash,
                        filename: ImString::new(entry.file_name().to_str().unwrap()),
                        relative_path,
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
                let e = entry.clone();
                TreeNode::new(&entry.filename)
                    .label(&entry.filename)
                    .leaf(true)
                    .selected(is_selected)
                    .build(&ui, || {
                        if ui.is_item_clicked(MouseButton::Left) {
                            ctx.editor_info.selected = e.path_hash;
                            ctx.inspector_info_write.set_current_entry(
                                Some(ctx.inspector_info_write.create_inspector(e.relative_path)));
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
    type SystemData = (ReadExpect<'a, EditorUIResources>,
                       WriteExpect<'a, AssetEditorResource>,
                       WriteExpect<'a, AssetInspectorResources>,
                        WriteExpect<'a, AssetEditorEvents>);

    fn run(&mut self, (editor_res, mut info, mut inspector_data, mut events): Self::SystemData) {
        if editor_res.all_opened_views.contains(VIEW_TOGGLE_ID) {
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

        events.clear();
    }
}

pub trait AssetInspectorFactory : Send + Sync {
    fn create(&self, path: PathBuf) -> Box<dyn AssetInspector>;
}

pub struct AssetInspectEntry {
    path: PathBuf,
    inspector: Box<dyn AssetInspector>,
    just_opened: bool,
}

pub struct AssetInspectContext<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub events: &'a mut AssetEditorEvents
}

pub trait AssetInspector : Send + Sync {

    fn display(&mut self, ui: AssetInspectContext);
    fn close(&self) {}

}

struct DefaultInspector {
}

impl AssetInspector for DefaultInspector {
    fn display(&mut self, ctx: AssetInspectContext) {
        ctx.ui.text("Unsupported format");
    }
}

pub struct SerializeConfigInspector<T, TInspect = T>
    where T: Send + Sync + Serialize + DeserializeOwned,
          TInspect: InspectRenderDefault<T> {
    repr: T,
    path: PathBuf,
    marker: PhantomData<TInspect>,
    debounce_marker: Option<Instant>
}

impl<T, TInspect> SerializeConfigInspector<T, TInspect>
    where T: Send + Sync + Serialize + DeserializeOwned,
    TInspect: InspectRenderDefault<T> {

    pub fn load(path: PathBuf) -> Self {
        let rpath = path.to_str().unwrap().to_string();
        let repr: T = serde_json::from_str(&load_asset::<String>(&rpath).unwrap()).unwrap();

        Self {
            path, repr,
            marker: PhantomData,
            debounce_marker: None
        }
    }

}

impl<T, TInspect> SerializeConfigInspector<T, TInspect>
    where T: Send + Sync + Serialize + DeserializeOwned,
    TInspect: InspectRenderDefault<T> + Send + Sync {

    fn save(&self) {
        let serialized_json = serde_json::to_string_pretty(&self.repr)
            .expect("Serialize failed");

        let rpath = self.path.to_str().unwrap().to_string();
        let fs_path = get_fs_path(&rpath);

        fs::write(&fs_path, serialized_json).expect("Write file failed");
        info!("Write to {:?}", fs_path);
    }

}

impl<T, TInspect> AssetInspector for SerializeConfigInspector<T, TInspect>
    where T: Send + Sync + Serialize + DeserializeOwned,
    TInspect: InspectRenderDefault<T> + Send + Sync {
    fn display(&mut self, ctx: AssetInspectContext) {
        if TInspect::render_mut(&mut [&mut self.repr], "test", &ctx.ui, &InspectArgsDefault::default()) {
            self.debounce_marker = Some(Instant::now())
        }

        if let Some(change_time) = self.debounce_marker {
            let dt = Instant::now() - change_time;
            if dt.as_secs_f32() > 1.0 {
                self.debounce_marker = None;
                self.save();
            }
        }
    }

    fn close(&self) {
        if self.debounce_marker.is_some() {
            self.save();
        }
    }
}

pub struct SerializeConfigInspectorFactory<T, TInspect = T>
    where T: Send + Sync + Serialize + DeserializeOwned,
          TInspect: Send + Sync + InspectRenderDefault<T> {
    marker: PhantomData<(T, TInspect)>,
}

impl<T, TInspect> SerializeConfigInspectorFactory<T, TInspect>
    where T: Send + Sync + Serialize + DeserializeOwned,
          TInspect: InspectRenderDefault<T> + Send + Sync {
    pub fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<T, TInspect> AssetInspectorFactory for SerializeConfigInspectorFactory<T, TInspect>
    where T: 'static + Send + Sync + Serialize + DeserializeOwned,
          TInspect: 'static + InspectRenderDefault<T> + Send + Sync {
    fn create(&self, path: PathBuf) -> Box<dyn AssetInspector> {
        Box::new(SerializeConfigInspector::<T, TInspect>::load(path))
    }
}

pub struct InspectorFactoryEntry {
    extension: String,
    factory: Box<dyn AssetInspectorFactory>
}

pub struct AssetInspectorResources {
    handlers: Vec<InspectorFactoryEntry>,
    current: Option<AssetInspectEntry>,
    // pinned: Vec<AssetInspectEntry>
}

impl AssetInspectorResources {

    pub fn set_current_entry(&mut self, entry: Option<AssetInspectEntry>) {
        if let Some(prev_entry) = self.current.take() {
            prev_entry.inspector.close();
        }

        self.current = entry;
    }

}

impl AssetInspectorResources {

    pub fn add_factory<T: AssetInspectorFactory + 'static>(&mut self, ext: &str, factory: T) {
        self.handlers.push(InspectorFactoryEntry {
            extension: ext.to_string(),
            factory: Box::new(factory)
        })
    }

    fn create_inspector(&self, path: PathBuf) -> AssetInspectEntry {
        for handler in &self.handlers {
            if path.file_name().unwrap().to_str().unwrap().ends_with(&handler.extension) {
                return AssetInspectEntry {
                    path: path.clone(),
                    inspector: handler.factory.create(path),
                    just_opened: true
                }
            }
        }

        AssetInspectEntry {
            path,
            inspector: Box::new(DefaultInspector {}),
            just_opened: true
        }
    }

}

impl AssetInspectorResources {

    pub fn new() -> Self {
        Self {
            handlers: vec![],
            current: None,
            // pinned: vec![]
        }
    }

}

pub(crate) struct InspectorSystem;

fn _show_inspector(ui: &Ui, events: &mut AssetEditorEvents, item: &mut AssetInspectEntry) {
    let mut window =
        Window::new(im_str!("Asset Inspector"))
            .size([300., 400.], Condition::FirstUseEver);

    if item.just_opened {
        item.just_opened = false;
        window = window.focused(true);
    }

    window
        .build(ui, || {
            let title = item.path.to_str().unwrap();
            ui.text_colored(Color::mono(0.8).into(), title);

            let ctx = AssetInspectContext {
                ui,
                events
            };
            item.inspector.display(ctx);
        });
}

impl<'a> System<'a> for InspectorSystem {
    type SystemData = (ReadExpect<'a, EditorUIResources>, WriteExpect<'a, AssetEditorEvents>, WriteExpect<'a, AssetInspectorResources>);

    fn run(&mut self, (editor_res, mut events, mut data): Self::SystemData) {
        if !editor_res.all_opened_views.contains(VIEW_TOGGLE_ID) {
            return
        }
        super::with_frame(|ui| {
            if let Some(current) = &mut data.current {
                _show_inspector(ui, &mut *events, current);
            }
        });
    }
}

pub const VIEW_TOGGLE_ID: &str = "asset_editor";
