use std::{default, f32::consts::PI, time::Duration};

use bevy::{
    asset::AssetMetaCheck,
    prelude::*,
    window::{CursorGrabMode, PresentMode, PrimaryWindow, WindowTheme},
};
use bevy_xpbd_2d::{math::Scalar, parry::na::ComplexField, prelude::*};

fn main() {
    App::new()
        // Never attempts to look up meta files. The default meta configuration will be used for each asset.
        .insert_resource(AssetMetaCheck::Never)
        .insert_resource(ClearColor(
            Color::hex("#071c42").expect("a valid hex color"),
        ))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Survive".into(),
                resolution: (1280.0, 720.0).into(),
                present_mode: PresentMode::AutoVsync,
                // Tells wasm to resize the window according to the available canvas
                fit_canvas_to_parent: true,
                // Tells wasm not to override default event handling, like F5, Ctrl+R etc.
                prevent_default_event_handling: false,
                window_theme: Some(WindowTheme::Dark),
                enabled_buttons: bevy::window::EnabledButtons {
                    maximize: false,
                    ..Default::default()
                },
                // This will spawn an invisible window
                // The window will be made visible in the make_visible() system after 3 frames.
                // This is useful when you want to avoid the white window that shows up before the GPU is ready to render the app.
                visible: true,
                ..default()
            }),
            ..default()
        }))
        //add framepacing to help with input lag
        .add_plugins(bevy_framepace::FramepacePlugin)
        .add_systems(Startup, setup)
        .init_resource::<MousePosition>()
        .add_systems(PreUpdate, update_mouse_position_system)
        .add_systems(Update, grab_mouse)
        .add_systems(Update, look_at_mouse)
        .add_systems(Update, proto_input)
        .add_event::<InputAction>()
        .add_systems(Update, movement.after(proto_input))
        .add_systems(Update, update_weapons.after(proto_input))
        .add_event::<SpawnLaserEvent>()
        .add_systems(Update, laser_spawner.after(update_weapons))
        //physics
        .add_plugins(PhysicsPlugins::default())
        //no gravity
        .insert_resource(Gravity(Vec2::ZERO))
        .add_plugins(PhysicsDebugPlugin::default())
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
            transform: Transform {
                scale: Vec3::splat(0.5),
                ..Default::default()
            },
            ..Default::default()
        },
        Ship,
        LookAtMouse,
        ShipControllerBundle::default(),
        laserWeaponBundle::default(),
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
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        let mut window = windows.single_mut();

        if mouse.just_pressed(MouseButton::Left) {
            window.cursor.visible = false;
            window.cursor.grab_mode = CursorGrabMode::Confined;
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
                // info!("diff: {}, angle: {}", diff, angle);
                entity_transform.rotation = Quat::from_rotation_z(angle);
            }
        }
        Err(_) => {
            info!("no mouse entity, so we cant look at it");
        }
    }
}

/// An event sent for a movement input action.
#[derive(Event)]
pub enum InputAction {
    Move(Vec2),
    Fire,
}

#[derive(Component)]
pub struct MovementAcceleration(Scalar);
/// The damping factor used for slowing down movement.
#[derive(Component)]
pub struct MovementDampingFactor(Scalar);
//ship controller bundle
#[derive(Bundle)]
pub struct ShipControllerBundle {
    rigid_body: RigidBody,
    collider: Collider,
    acceleration: MovementAcceleration,
    lineardamping: LinearDamping,
    layer: CollisionLayers,
}

impl Default for ShipControllerBundle {
    fn default() -> Self {
        Self {
            rigid_body: RigidBody::Dynamic,
            collider: Collider::ball(40.0),
            acceleration: MovementAcceleration(10.0 * 128.0),
            lineardamping: LinearDamping(0.99),
            layer: CollisionLayers::new([Layer::Blue], [Layer::Red]),
        }
    }
}

fn proto_input(
    mut input_event_writer: EventWriter<InputAction>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    let left = keyboard_input.any_pressed([KeyCode::A, KeyCode::Left]);
    let right = keyboard_input.any_pressed([KeyCode::D, KeyCode::Right]);
    let up = keyboard_input.any_pressed([KeyCode::W, KeyCode::Up]);
    let down = keyboard_input.any_pressed([KeyCode::S, KeyCode::Down]);
    let horizontal = match (left, right) {
        (true, true) => 0.0,
        (true, false) => -1.0,
        (false, true) => 1.0,
        (false, false) => 0.0,
    };

    let vertical = match (up, down) {
        (true, true) => 0.0,
        (true, false) => 1.0,
        (false, true) => -1.0,
        (false, false) => 0.0,
    };
    // info!("h: {}, v:{}", horizontal, vertical);
    if horizontal.abs() > 0.0 || vertical.abs() > 0.0 {
        input_event_writer.send(InputAction::Move(Vec2 {
            x: horizontal,
            y: vertical,
        }))
    }
    let fire = keyboard_input.any_pressed([KeyCode::Space]);
    if fire {
        input_event_writer.send(InputAction::Fire);
    }
}

/// Responds to [`InputAction`] events and moves character controllers accordingly.
fn movement(
    time: Res<Time>,
    mut input_event_reader: EventReader<InputAction>,
    mut ship_query: Query<(&mut LinearVelocity, &MovementAcceleration), With<Ship>>,
) {
    let delta_time = time.delta_seconds();

    for event in input_event_reader.read() {
        for mut ship in ship_query.iter_mut() {
            match event {
                InputAction::Move(direction) => {
                    //testing
                    let thrust = *direction * (ship.1 .0 * delta_time);
                    ship.0 .0 += thrust;
                }
                InputAction::Fire => {}
            }
        }
    }
}
//this is in rounds per minute
#[derive(Component)]
pub struct RateOfFire(i8);

#[derive(Component)]
struct FireTimer(Timer);

//bundle for the laser weapon
#[derive(Bundle)]
pub struct laserWeaponBundle {
    rate_of_fire: RateOfFire,
    fire_timer: FireTimer,
}

impl Default for laserWeaponBundle {
    fn default() -> Self {
        Self {
            rate_of_fire: RateOfFire(60),
            fire_timer: FireTimer(Timer::new(Duration::from_secs(1), TimerMode::Once)),
        }
    }
}

/// An event sent for a firing a laser
#[derive(Event)]
pub struct SpawnLaserEvent {
    origin: Transform,
}

fn update_weapons(
    time: Res<Time>,
    mut input_event_reader: EventReader<InputAction>,
    mut ship_query: Query<(&mut FireTimer, &RateOfFire, &Transform, &LinearVelocity), With<Ship>>,
    mut fire_laser_event_writer: EventWriter<SpawnLaserEvent>,
) {
    let delta_time = time.delta();

    for event in input_event_reader.read() {
        for mut ship in ship_query.iter_mut() {
            //always tick the weapons timer
            ship.0 .0.tick(delta_time);
            match event {
                InputAction::Move(_) => {}
                InputAction::Fire => {
                    //if the timer is finished we can pew
                    if ship.0 .0.finished() {
                        fire_laser_event_writer.send(SpawnLaserEvent {
                            origin: ship.2.clone(),
                        });
                        ship.0 .0.reset();
                    }
                }
            }
        }
    }
}
#[derive(Component)]
pub struct Lifetime(Timer);

// Define the collision layers
#[derive(PhysicsLayer)]
enum Layer {
    Blue,
    Red,
}

#[derive(Bundle)]
pub struct LaserBoltBundle {
    sprite_bundle: SpriteBundle,
    rigid_body: RigidBody,
    collider: Collider,
    linear_velocity: LinearVelocity,
    lifetime: Lifetime,
    layer: CollisionLayers,
}

impl Default for LaserBoltBundle {
    fn default() -> Self {
        Self {
            sprite_bundle: Default::default(),
            rigid_body: RigidBody::Dynamic,
            collider: Collider::capsule(40.0, 6.0),
            linear_velocity: Default::default(),
            lifetime: Lifetime(Timer::new(Duration::from_secs(5), TimerMode::Once)),
            layer: CollisionLayers::new([Layer::Blue], [Layer::Red]),
        }
    }
}

fn laser_spawner(
    mut reader: EventReader<SpawnLaserEvent>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for event in reader.read() {
        info!("pew");
        //spawn laser bolt
        //speed
        let speed = 500.0;
        let euler_rots = event.origin.rotation.to_euler(EulerRot::XYZ);
        let z_rot = euler_rots.2 + PI / 2.0;
        let x = z_rot.cos();
        let y = z_rot.sin();
        commands.spawn(LaserBoltBundle {
            sprite_bundle: SpriteBundle {
                texture: asset_server.load("lasers/laserBlue01.png"),
                transform: event.origin,
                ..Default::default()
            },
            linear_velocity: LinearVelocity(Vec2 { x: x, y: y } * speed),
            ..default()
        });
    }
}
