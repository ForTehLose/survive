use std::f32::consts::PI;

use bevy::{
    asset::AssetMetaCheck,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};

fn main() {
    App::new()
        // Never attempts to look up meta files. The default meta configuration will be used for each asset.
        .insert_resource(AssetMetaCheck::Never)
        .insert_resource(ClearColor(
            Color::hex("#071c42").expect("a valid hex color"),
        ))
        .add_plugins(DefaultPlugins)
        //add framepacing to help with input lag
        .add_plugins(bevy_framepace::FramepacePlugin)
        .add_systems(Startup, setup)
        .init_resource::<MousePosition>()
        .add_systems(PreUpdate, update_mouse_position_system)
        .add_systems(Update, grab_mouse)
        .add_systems(Update, look_at_mouse)
        .add_systems(Update, proto_move)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    //spawn camera
    commands.spawn((Camera2dBundle::default(), MainCamera));
    //spawn mouse sprite
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("crosshair/crossair_white.png"),
            ..Default::default()
        },
        Mouse,
    ));
    //spawn ship entity
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("playerShip1_orange.png"),
            ..Default::default()
        },
        Ship,
        LookAtMouse,
    ));
}

/// We will store the world position of the mouse cursor here.
#[derive(Resource, Default)]
struct MousePosition(Vec2);

/// Used to help identify our main camera
#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct Mouse;

#[derive(Component)]
struct Ship;

fn update_mouse_position_system(
    mut mouse_position_resource: ResMut<MousePosition>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut mouse_entity_query: Query<&mut Transform, With<Mouse>>,
) {
    // get the camera info and transform
    // assuming there is exactly one main camera entity, so Query::single() is OK
    let (camera, camera_transform) = camera_query.single();

    // There is only one primary window, so we can similarly get it from the query:
    let window = window_query.single();

    //try to get the mouse entity
    let mouse_entity = mouse_entity_query.get_single_mut();

    // check if the cursor is inside the window and get its position
    // then, ask bevy to convert into world coordinates, and truncate to discard Z
    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor))
        .map(|ray| ray.origin.truncate())
    {
        //print what we found
        // info!("World coords: {}/{}", world_position.x, world_position.y);
        //update the resource
        mouse_position_resource.0 = world_position;
        //if we have an entity update it
        match mouse_entity {
            Ok(mut mouse) => {
                mouse.translation = world_position.extend(0.0);
            }
            Err(_) => {
                info!("no mouse found");
            }
        }
    }
}

// This system grabs the mouse when the left mouse button is pressed
// and releases it when the escape key is pressed
fn grab_mouse(
    mut windows: Query<&mut Window>,
    mouse: Res<Input<MouseButton>>,
    key: Res<Input<KeyCode>>,
) {
    #[cfg(not(target_os = "wasm32-unknown-unknown"))]
    {
        let mut window = windows.single_mut();

        if mouse.just_pressed(MouseButton::Left) {
            window.cursor.visible = false;
            window.cursor.grab_mode = CursorGrabMode::Locked;
        }

        if key.just_pressed(KeyCode::Escape) {
            window.cursor.visible = true;
            window.cursor.grab_mode = CursorGrabMode::None;
        }
    }
}

#[derive(Component)]
struct LookAtMouse;

fn look_at_mouse(
    mouse_entity_query: Query<&Transform, With<Mouse>>,
    mut observers_query: Query<&mut Transform, (With<LookAtMouse>, Without<Mouse>)>,
) {
    //get the mouse once
    let mouse_entity = mouse_entity_query.get_single();
    match mouse_entity {
        Ok(mouse_transform) => {
            for mut entity_transform in observers_query.iter_mut() {
                let diff = mouse_transform.translation.xy() - entity_transform.translation.xy();
                let angle = diff.y.atan2(diff.x) - PI / 2.0;
                info!("diff: {}, angle: {}", diff, angle);
                entity_transform.rotation = Quat::from_rotation_z(angle);
            }
        }
        Err(_) => {
            info!("no mouse entity, so we cant look at it");
        }
    }
}

fn proto_move(mut ship_query: Query<&mut Transform, With<Ship>>, key: Res<Input<KeyCode>>) {
    if key.pressed(KeyCode::W) {
        for mut ship_transform in ship_query.iter_mut() {
            ship_transform.translation.y += 1.0;
        }
    }
    if key.pressed(KeyCode::S) {
        for mut ship_transform in ship_query.iter_mut() {
            ship_transform.translation.y -= 1.0;
        }
    }
    if key.pressed(KeyCode::A) {
        for mut ship_transform in ship_query.iter_mut() {
            ship_transform.translation.x -= 1.0;
        }
    }
    if key.pressed(KeyCode::D) {
        for mut ship_transform in ship_query.iter_mut() {
            ship_transform.translation.x += 1.0;
        }
    }
}
