use std::fs;
use std::io;
use std::path::Path as Path;

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

pub fn load_asset<T>(path: &str) -> io::Result<T>
where T: LoadableAsset
{
    return T::read(path);
}

pub fn load_asset_local<T>(base_dir: &str, path: &str) -> io::Result<T>
where T: LoadableAsset
{
    let p = if base_dir.is_empty() {
        String::from(path)
    } else {
        format!("{}/{}", base_dir, path)
    };
    info!("path: {}", p);
    return T::read(p.as_str());
}

impl LoadableAsset for String {
    fn read(path: &str) -> io::Result<Self> {
        fs::read_to_string(get_fs_path(path))
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