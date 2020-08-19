use mu::{RuntimeBuilder, Module, InitData, InsertInfo, InitContext};
use mu::client::editor;
use mu::client::graphics;
use mu::client::graphics::GraphicsModule;
use mu::client::editor::EditorModule;
use specs::System;

fn main() {
    mu::asset::set_base_asset_path("./examples/asset");

    let runtime = RuntimeBuilder::new("Editor Example")
        .add_game_module(GraphicsModule)
        .add_game_module(EditorModule)
        .build();

    runtime.start();
}