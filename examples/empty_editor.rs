use mu::*;
use mu::client::graphics::GraphicsModule;
use mu::client::editor::EditorModule;

fn main() {
    mu::asset::set_base_asset_path("./examples/asset");

    let runtime = RuntimeBuilder::new("Editor Example")
        .add_game_module(GraphicsModule)
        .add_game_module(EditorModule { asset_path: Some("./examples/asset".to_string()) })
        .build();

    runtime.start();
}