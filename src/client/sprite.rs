use crate::{asset, Module, InitData, InsertInfo};
use crate::math::*;
use crate::asset::LoadableAsset;
use serde_json;
use serde::Deserialize;
use std::io;
use crate::client::graphics::Texture;
use crate::client::graphics;
use glium::Display;
use specs::{Component, VecStorage, System, ReadStorage};
use crate::ecs::Transform;
use std::collections::HashMap;
use uuid::Uuid;
use std::cell::RefCell;
use specs::Join;

#[derive(Clone, Deserialize)]
pub struct SpriteConfig {
    name: String,
    // https://serde.rs/remote-derive.html
    #[serde(with = "Vec2SerdeRef")]
    pos: Vec2,  // Center of the sprite, in pixel coordinates
    #[serde(with = "Vec2SerdeRef")]
    size: Vec2, // Size of the image, in pixel coordinates
    #[serde(with = "Vec2SerdeRef")]
    pivot: Vec2, // the pos of the pivot within the sprite (normalized 0-1 range)
}

#[derive(Deserialize)]
pub struct SpriteSheetConfig {
    texture: String,
    sprites: Vec<SpriteConfig>,
    ppu: u32,
    #[serde(skip)]
    _path: String,
}

impl LoadableAsset for SpriteSheetConfig {
    fn read(path: &str) -> io::Result<Self> {
        let text = asset::load_asset::<String>(path)?;
        let mut config: SpriteSheetConfig = serde_json::from_str(&text)?;

        config._path = asset::get_dir(path);
        Ok(config)
    }
}

#[derive(Clone)]
pub struct Sprite {
    pub config: SpriteConfig,
    pub sheet_uuid: Uuid,
}

pub struct SpriteSheet {
    pub sprites: Vec<Sprite>,
    pub texture: Texture,
    pub uuid: Uuid
}

impl SpriteSheet {
    fn as_ref(&self) -> SpriteSheetRef {
        SpriteSheetRef {
            uuid: self.uuid,
            sprites: (&self.sprites).into_iter()
                .enumerate()
                .map(|(pos, e)| {
                    SpriteRef {
                        name: e.config.name.clone(),
                        idx: pos,
                        sheet_uuid: self.uuid
                    }
                })
                .collect()
        }
    }
}

// 由于texture不能放到Component里（无法跨线程），且太重量级，在加载完后以及Component层使用SpriteRef
// 在渲染时才由SpriteRef拿回Sprite，利用Texture进行实际绘制

#[derive(Clone)]
pub struct SpriteRef {
    pub sheet_uuid: Uuid,
    pub idx: usize,
    pub name: String,
}

pub struct SpriteSheetRef {
    pub sprites: Vec<SpriteRef>,
    pub uuid: Uuid
}

pub fn load_sprite_sheet(display: &Display, path: &str) -> io::Result<SpriteSheetRef> {
    let config: SpriteSheetConfig = asset::load_asset(path)?;
    let texture: Texture = graphics::load_texture(display,
                                                  &asset::get_asset_path_local(&config._path, &config.texture));
    let uuid = Uuid::new_v4();

    let sprites: Vec<Sprite> = (&config.sprites).into_iter()
        .map(|x| Sprite { config: x.clone(), sheet_uuid: uuid })
        .collect();

    let sheet = SpriteSheet {
        texture,
        sprites,
        uuid
    };
    let sheet_ref = sheet.as_ref();
    LOADED_SPRITE_SHEETS.with(|ref_cell| {
        ref_cell.borrow_mut().insert(uuid, sheet)
    });

    Ok(sheet_ref)
}

pub fn unload_sprite_sheet(uuid: Uuid) {
    LOADED_SPRITE_SHEETS.with(|ref_cell| {
        ref_cell.borrow_mut().remove(&uuid);
    });
}

pub struct SpriteRenderer {
    pub sprite: SpriteRef,
}

impl Component for SpriteRenderer {
    type Storage = VecStorage<Self>;
}

pub struct SpriteModule;

impl Module for SpriteModule {
    fn init(&self, init_data: &mut InitData) {
        init_data.dispatch_thread_local(
        InsertInfo::new("sprite")
                .before(&[graphics::DEP_RENDER_TEARDOWN])
                .after(&[graphics::DEP_RENDER_SETUP])
                .order(graphics::render_order::OPAQUE),
            |f| f.insert_thread_local(SpriteRenderSystem));
    }
}


struct SpriteRenderSystem;

impl<'a> System<'a> for SpriteRenderSystem {
    type SystemData = (ReadStorage<'a, SpriteRenderer>, ReadStorage<'a, Transform>);

    fn run(&mut self, (sr_vec, trans_vec): Self::SystemData) {
        for (trans, sr) in (&sr_vec, &trans_vec).join() {
            // DO the rendering
        }
    }
}

// TODO: Make this instanced per Runtime
thread_local! {
static LOADED_SPRITE_SHEETS: RefCell<HashMap<Uuid, SpriteSheet>> = RefCell::new(HashMap::new());
}
