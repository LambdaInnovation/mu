use std::fs;
use std::io;
use std::path::Path as Path;
use std::collections::HashMap;
use std::cell::{RefCell, RefMut};
use std::any::{TypeId, Any, type_name};
use std::borrow::{Borrow};
use std::marker::PhantomData;
use std::sync::Arc;
use std::ops::{SubAssign, AddAssign};
use std::sync::atomic::{AtomicU32, Ordering};

static mut BASE_ASSET_PATH: &str = "./assets/";

pub trait LoadableAsset
where Self : Sized {
    fn read(path: &str) -> io::Result<Self>;
}

pub fn get_dir(path: &str) -> String {
    match path.find('/') {
        Some(ix) => String::from(&path[0..ix]),
        None => String::new()
    }
}

#[inline]
pub fn load_asset<T>(path: &str) -> io::Result<T>
where T: LoadableAsset
{
    info!("load_asset: {:?}", &path);
    return T::read(path);
}

#[inline]
pub fn load_asset_local<T>(base_dir: &str, path: &str) -> io::Result<T>
where T: LoadableAsset
{
    let p = get_asset_path_local(base_dir, path);
    info!("load_asset: {:?}", &p);
    return T::read(p.as_str());
}

#[inline]
pub fn get_asset_path_local(base_dir: &str, path: &str) -> String {
    if base_dir.is_empty() {
        String::from(path)
    } else {
        format!("{ }/{}", base_dir, path)
    }
}

impl LoadableAsset for String {
    fn read(path: &str) -> io::Result<Self> {
        fs::read_to_string(get_fs_path(path))
    }
}

impl LoadableAsset for Vec<u8> {
    fn read(path: &str) -> io::Result<Self> {
        fs::read(get_fs_path(path))
    }
}

pub fn set_base_asset_path(path: &'static str) {
    unsafe {
        BASE_ASSET_PATH = path;
    }
}

fn get_fs_path(path: &str) -> Box<Path> {
    return Path::new(unsafe { BASE_ASSET_PATH }).join(path).into_boxed_path();
}

pub struct ResourceRef<T: 'static> {
    idx: usize,
    type_id: TypeId,
    ref_cnt: Arc<AtomicU32>,
    marker: PhantomData<T>
}

impl<T: 'static> PartialEq for ResourceRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx
    }
}

impl<T: 'static> Eq for ResourceRef<T> {
}

// PhantomData 只当做一个类型标记，实际上能够跨线程同步
unsafe impl<T: 'static> Send for ResourceRef<T> {}
unsafe impl<T: 'static> Sync for ResourceRef<T> {}

impl<T: 'static> Drop for ResourceRef<T> {
    fn drop(&mut self) {
        let remain = self.ref_cnt.fetch_sub(1, Ordering::SeqCst);
        // info!("Sub resCount {:?} = {}", std::any::type_name::<T>(), remain);
    }
}

impl<T: 'static> Clone for ResourceRef<T> {
    fn clone(&self) -> Self {
        let mut ret = Self {
            idx: self.idx,
            type_id: self.type_id,
            marker: PhantomData,
            ref_cnt: self.ref_cnt.clone()
        };

        let cnt = self.ref_cnt.fetch_add(1, Ordering::SeqCst);
        // info!("Add resCount {:?} = {}", std::any::type_name::<T>(), cnt);

        ret
    }
}

struct ResourceEntry<T> {
    resource: T,
    ref_cnt: Arc<AtomicU32>
}

pub struct ResourcePool<T> where T: 'static {
    entries: Vec<Option<ResourceEntry<T>>>,
    free_indices: Vec<usize>,
}

trait LocalResourcePool {
    fn cleanup(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> ResourcePool<T> where T: 'static {

    pub fn new() -> Self {
        Self {
            entries: vec![],
            free_indices: vec![]
        }
    }

    pub fn add(&mut self, res: T) -> ResourceRef<T> {
        let ref_cnt = Arc::new(AtomicU32::new(1));
        let resource_entry = ResourceEntry {
            resource: res,
            ref_cnt: ref_cnt.clone()
        };
        let idx = if self.free_indices.is_empty() {
            self.entries.push(Some(resource_entry));
            self.entries.len() - 1
        } else {
            let idx = self.free_indices.remove(self.free_indices.len() - 1);
            self.entries[idx] = Some(resource_entry);
            idx
        };

        ResourceRef {
            idx,
            type_id: TypeId::of::<T>(),
            ref_cnt,
            marker: PhantomData
        }
    }

    pub fn get(&self, res_ref: &ResourceRef<T>) -> &T {
        & (&self.entries[res_ref.idx]).as_ref().unwrap().resource
    }

    pub fn get_mut(&mut self, res_ref: &ResourceRef<T>) -> &mut T {
        &mut (&mut self.entries[res_ref.idx]).as_mut().unwrap().resource
    }
}

impl<T> LocalResourcePool for ResourcePool<T> where T: 'static {
    fn cleanup(&mut self) {
        for (ix, item) in (&mut self.entries).iter_mut().enumerate() {
            let need_remove = match item {
                Some(x) if x.ref_cnt.load(Ordering::SeqCst) == 0 => true,
                _ => false
            };

            if need_remove {
                item.take();
                self.free_indices.push(ix);
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub fn add_local_resource<T>(res: T) -> ResourceRef<T>
where T: 'static
{
    ALL_RESOURCES.with(|ref_cell| {
        ref_cell.borrow_mut().add(res)
    })
}

pub fn with_local_resource_mgr<F, R>(f: F) -> R
    where F: FnOnce(&mut LocalResourcesManager) -> R
{
    ALL_RESOURCES.with(|ref_cell| {
        let mut mgr_ref = ref_cell.borrow_mut();
        f(&mut mgr_ref)
    })
}

/// 帧末清理引用数为0的thread local资源
pub fn cleanup_local_resources() {
    ALL_RESOURCES.with(|ref_cell| {
        ref_cell.borrow_mut().cleanup();
    })
}

pub struct LocalResourcesManager {
    map: HashMap<TypeId, Box<dyn LocalResourcePool>>
}

impl LocalResourcesManager {
    fn new() -> Self {
        Self {
            map: HashMap::new()
        }
    }

    pub fn add<T: 'static>(&mut self, res: T) -> ResourceRef<T> {
        let type_id = TypeId::of::<T>();
        if !self.map.contains_key(&type_id) {
            let res_pool: ResourcePool<T> = ResourcePool::new();
            self.map.insert(type_id, Box::new(res_pool));
        }

        let pool: &mut ResourcePool<T> = self.map.get_mut(&type_id).unwrap().as_any_mut().downcast_mut().unwrap();
        pool.add(res)
    }

    pub fn get<T: 'static>(&self, res_ref: &ResourceRef<T>) -> &T {
        let pool: &ResourcePool<T> = self.map
            .get(&res_ref.type_id).unwrap().as_any().downcast_ref().unwrap();
        pool.get(res_ref)
    }

    pub fn get_mut<T: 'static>(&mut self, res_ref: &ResourceRef<T>) -> &mut T {
        let mut pool: &mut ResourcePool<T> = self.map
            .get_mut(&res_ref.type_id).unwrap().as_any_mut().downcast_mut().unwrap();
        pool.get_mut(&res_ref)
    }

    pub fn cleanup(&mut self) {
        for (_, v) in &mut self.map {
            v.cleanup();
        }
    }
}

thread_local! {
static ALL_RESOURCES: RefCell<LocalResourcesManager> = RefCell::new(LocalResourcesManager::new());
}