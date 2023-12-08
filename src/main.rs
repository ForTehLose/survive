use bevy::{asset::AssetMetaCheck, prelude::*};

fn main() {
    App::new()
        // Never attempts to look up meta files. The default meta configuration will be used for each asset.
        .insert_resource(AssetMetaCheck::Never)
        .insert_resource(ClearColor(
            Color::hex("#071c42").expect("a valid hex color"),
        ))
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn(SpriteBundle {
        texture: asset_server.load("playerShip1_orange.png"),
        ..Default::default()
    });
}
