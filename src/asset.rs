use std::fs;
use std::io;
use std::path::Path as Path;
use std::collections::HashMap;
use std::cell::{RefCell, RefMut};
use std::any::{TypeId, Any};
use std::borrow::BorrowMut;
use std::marker::PhantomData;

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

pub struct ResourceRef<T> {
    idx: usize,
    type_id: TypeId,
    marker: PhantomData<T>
}

impl<T> Drop for ResourceRef<T> {
    fn drop(&mut self) {
        // TODO: Reduce ref count
    }
}

impl<T> Clone for ResourceRef<T> {
    fn clone(&self) -> Self {
        // TODO: Add ref count

        Self {
            idx: self.idx,
            type_id: self.type_id,
            marker: PhantomData
        }
    }
}

struct ResourceEntry<T> {
    resource: T,
    ref_cnt: u32
}

struct ResourcePool<T> where T: 'static {
    entries: Vec<Option<ResourceEntry<T>>>,
    free_indices: Vec<usize>,
}

impl<T> ResourcePool<T> where T: 'static {

    fn new() -> Self {
        Self {
            entries: vec![],
            free_indices: vec![]
        }
    }

    fn add(&mut self, res: T) -> ResourceRef<T> {
        let resource_entry = ResourceEntry {
            resource: res,
            ref_cnt: 1
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
            marker: PhantomData
        }
    }

}

pub fn add_resource<T>(res: T) -> ResourceRef<T>
where T: 'static
{
    let type_id = TypeId::of::<T>();
    ALL_RESOURCES.with(|ref_cell| {
        let mut map = ref_cell.borrow_mut();
        if !map.contains_key(&type_id) {
            let hash_map: HashMap<TypeId, ResourcePool<T>> = HashMap::new();
            map.insert(type_id, Box::new(hash_map));
        }

        let pool: &mut ResourcePool<T> = map.get_mut(&type_id).unwrap().downcast_mut().unwrap();
        pool.add(res)
    })
}

pub fn with_resource<T, F>(res_ref: &ResourceRef<T>, f: F)
where F: FnOnce(&mut T), T: 'static
{
    let type_id = TypeId::of::<T>();
    assert_eq!(type_id, res_ref.type_id);

    ALL_RESOURCES.with(|ref_cell| {
        let mut map = ref_cell.borrow_mut();
        let pool: &mut ResourcePool<T> = map.get_mut(&type_id).unwrap().downcast_mut().unwrap();
        let res = &mut pool.entries[res_ref.idx].as_mut().unwrap().resource;
        f(res);
    });
}

pub fn get_resource<T>(res_ref: &ResourceRef<T>) -> &T {
    // let ref_mut = ALL_RESOURCES.w
    // ref_mut
}

thread_local! {
static ALL_RESOURCES: RefCell<HashMap<TypeId, Box<dyn Any>>> = RefCell::new(HashMap::new());
}