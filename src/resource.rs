/// A `Resource` is a managed object of any type. It's often used to share instances of data
/// within game.
///
/// For objects that can be sent between threads, use `ResManager` resource in world to access resources.
///
/// For non-threadable objects, use `add_local_resource`, `cleanup_local_resources`, etc. methods to
/// access resources stored in current thread.
use crate::asset::*;
use std::marker::PhantomData;
use std::sync::Arc;
use std::any::{TypeId, Any, type_name};
use std::io;
use std::cell::{RefCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

pub type LocalResManager = ResourceManager<dyn ResPool>;
pub type ResManager = ResourceManager<dyn ThreadedResPool>;

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

impl<T: 'static> Eq for ResourceRef<T> {}

// PhantomData 只当做一个类型标记，实际上能够跨线程同步
unsafe impl<T: 'static> Send for ResourceRef<T> {}
unsafe impl<T: 'static> Sync for ResourceRef<T> {}

impl<T: 'static> Drop for ResourceRef<T> {
    fn drop(&mut self) {
        self.ref_cnt.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<T: 'static> Clone for ResourceRef<T> {
    fn clone(&self) -> Self {
        let ret = Self {
            idx: self.idx,
            type_id: self.type_id,
            marker: PhantomData,
            ref_cnt: self.ref_cnt.clone()
        };

        self.ref_cnt.fetch_add(1, Ordering::SeqCst);

        ret
    }
}

pub type ResourceKey = u64;

struct ResourceEntry<T> {
    resource: T,
    ref_cnt: Arc<AtomicU32>
}

pub struct ResourcePool<T> where T: 'static {
    entries: Vec<Option<ResourceEntry<T>>>,
    free_indices: Vec<usize>,
    res_mapping: HashMap<ResourceKey, usize>
}

impl<T: 'static + Send + Sync> ThreadedResPool for ResourcePool<T> {}

pub trait ResPool {
    fn cleanup(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait ThreadedResPool: ResPool + Send + Sync {}

impl<T: 'static + LoadableAsset> ResourcePool<T> {

    pub fn load_asset(&mut self, path: &str) -> io::Result<ResourceRef<T>> {
        let asset = load_asset(path)?;
        Ok(self.add(asset))
    }

}

impl<T> ResourcePool<T> where T: 'static {

    pub fn new() -> Self {
        Self {
            entries: vec![],
            free_indices: vec![],
            res_mapping: HashMap::new()
        }
    }

    pub fn get_by_key(&self, k: ResourceKey) -> Option<ResourceRef<T>> {
        self.res_mapping.get(&k)
            .map(|v| {
                let entry = self.entries[*v].as_ref().unwrap();
                entry.ref_cnt.fetch_add(1, Ordering::SeqCst);

                ResourceRef {
                    idx: *v,
                    type_id: TypeId::of::<T>(),
                    ref_cnt: entry.ref_cnt.clone(),
                    marker: PhantomData
                }
            })
    }

    pub fn add_by_key(&mut self, res: T, key: ResourceKey) -> ResourceRef<T> {
        let r = self.add(res);
        self.res_mapping.insert(key, r.idx);
        r
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

impl<T> ResPool for ResourcePool<T> where T: 'static {
    fn cleanup(&mut self) {
        let mut has_remove = false;
        for (ix, item) in (&mut self.entries).iter_mut().enumerate() {
            let need_remove = match item {
                Some(x) if x.ref_cnt.load(Ordering::SeqCst) == 0 => true,
                _ => false
            };

            if need_remove {
                info!("Cleanup asset of type {}", type_name::<T>());
                item.take();
                self.free_indices.push(ix);
                has_remove = true;
            } else {
                // info!("type {} ptr={:?}", type_name::<T>(),
                //       (&item).as_ref().map(|x| x.ref_cnt.load(Ordering::SeqCst)));
            }
        }

        if has_remove {
            self.res_mapping = self.res_mapping.iter()
                .filter(|(_, v)| self.entries[**v].is_none())
                .map(|(k, v)| (*k, *v))
                .collect();
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
    where F: FnOnce(&mut ResourceManager<dyn ResPool>) -> R
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

pub struct ResourceManager<R: ResPool + ?Sized> {
    map: HashMap<TypeId, Box<R>>
}

impl ResourceManager<dyn ResPool> {

    pub fn add<T: 'static>(&mut self, res: T) -> ResourceRef<T> {
        let type_id = TypeId::of::<T>();
        if !self.map.contains_key(&type_id) {
            let res_pool: ResourcePool<T> = ResourcePool::new();
            self.map.insert(type_id, Box::new(res_pool));
        }

        let pool: &mut ResourcePool<T> = self.map.get_mut(&type_id).unwrap().as_any_mut().downcast_mut().unwrap();
        pool.add(res)
    }

    pub fn get_pool_mut<T: 'static>(&mut self) -> &mut ResourcePool<T> {
        let type_id = TypeId::of::<T>();
        if !self.map.contains_key(&type_id) {
            let res_pool: ResourcePool<T> = ResourcePool::new();
            self.map.insert(type_id.clone(), Box::new(res_pool));
        }
        self.map.get_mut(&type_id).unwrap().as_any_mut().downcast_mut().unwrap()
    }

    pub fn get_mut<T: 'static>(&mut self, res_ref: &ResourceRef<T>) -> &mut T {
        let pool: &mut ResourcePool<T> = self.get_pool_mut();
        pool.get_mut(&res_ref)
    }

}

impl ResourceManager<dyn ThreadedResPool> {

    pub fn add_by_key<T: 'static + Send + Sync>(&mut self, res: T, key: ResourceKey)
        -> ResourceRef<T> {
        let type_id = TypeId::of::<T>();
        if !self.map.contains_key(&type_id) {
            let res_pool: ResourcePool<T> = ResourcePool::new();
            self.map.insert(type_id, Box::new(res_pool));
        }

        let pool: &mut ResourcePool<T> = self.map.get_mut(&type_id).unwrap().as_any_mut().downcast_mut().unwrap();
        pool.add_by_key(res, key)
    }

    pub fn add<T: 'static + Send + Sync>(&mut self, res: T) -> ResourceRef<T> {
        let type_id = TypeId::of::<T>();
        if !self.map.contains_key(&type_id) {
            let res_pool: ResourcePool<T> = ResourcePool::new();
            self.map.insert(type_id, Box::new(res_pool));
        }

        let pool: &mut ResourcePool<T> = self.map.get_mut(&type_id).unwrap().as_any_mut().downcast_mut().unwrap();
        pool.add(res)
    }

    pub fn get_pool_mut<T: 'static + Send + Sync>(&mut self) -> &mut ResourcePool<T> {
        let type_id = TypeId::of::<T>();
        if !self.map.contains_key(&type_id) {
            let res_pool: ResourcePool<T> = ResourcePool::new();
            self.map.insert(type_id.clone(), Box::new(res_pool));
        }
        self.map.get_mut(&type_id).unwrap().as_any_mut().downcast_mut().unwrap()
    }

    pub fn get_mut<T: 'static + Send + Sync>(&mut self, res_ref: &ResourceRef<T>) -> &mut T {
        let pool: &mut ResourcePool<T> = self.get_pool_mut();
        pool.get_mut(&res_ref)
    }

}

impl<R: ResPool + ?Sized> ResourceManager<R> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new()
        }
    }

    pub fn get_pool<T: 'static>(&self) -> Option<&ResourcePool<T>> {
        self.map.get(&TypeId::of::<T>()).map(|x| x.as_any().downcast_ref().unwrap())
    }

    pub fn get<T: 'static>(&self, res_ref: &ResourceRef<T>) -> &T {
        let pool: &ResourcePool<T> = self.get_pool().unwrap();
        pool.get(res_ref)
    }

    pub fn get_by_key<T: 'static>(&self, key: ResourceKey) -> Option<ResourceRef<T>> {
        if let Some(pool) = self.get_pool::<T>() {
            pool.get_by_key(key)
        } else {
            None
        }
    }

    pub fn cleanup(&mut self) {
        for (_, v) in &mut self.map {
            v.cleanup();
        }
    }
}

thread_local! {
static ALL_RESOURCES: RefCell<ResourceManager<dyn ResPool>> = RefCell::new(ResourceManager::new());
}
