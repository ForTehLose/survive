use std::{f32::consts::PI, time::Duration};

use bevy::{
    asset::AssetMetaCheck,
    prelude::*,
    window::{CursorGrabMode, PresentMode, PrimaryWindow, WindowTheme},
};
use bevy_xpbd_2d::{math::Scalar, parry::na::ComplexField, prelude::*};
use rand::Rng;

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
        .add_systems(PostUpdate, update_lifetimes)
        .add_event::<SpawnAsteroidEvent>()
        .add_systems(Update, asteroid_spawner)
        .add_systems(Update, handle_collisions)
        .add_systems(Update, handle_destroyed_asteroids)
        .add_systems(Update, wrapper)
        //physics
        .add_plugins(PhysicsPlugins::default())
        //no gravity
        .insert_resource(Gravity(Vec2::ZERO))
        .add_plugins(PhysicsDebugPlugin::default())
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut asteroid_event_writer: EventWriter<SpawnAsteroidEvent>,
) {
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
        LaserWeaponBundle::default(),
    ));
    asteroid_event_writer.send(SpawnAsteroidEvent {
        origin: Transform {
            translation: Vec3 {
                x: 100.0,
                y: 100.0,
                z: 0.0,
            },
            ..default()
        },
        class: AsteroidClass::Big,
        velocity: LinearVelocity::default(),
        angular: AngularVelocity::default(),
    });
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
pub struct LaserWeaponBundle {
    rate_of_fire: RateOfFire,
    fire_timer: FireTimer,
}

impl Default for LaserWeaponBundle {
    fn default() -> Self {
        Self {
            rate_of_fire: RateOfFire(60),
            fire_timer: FireTimer(Timer::new(
                Duration::from_secs_f32(60.0 / 120.0),
                TimerMode::Once,
            )),
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
#[derive(Component)]
pub struct Laser;

#[derive(Bundle)]
pub struct LaserBoltBundle {
    sprite_bundle: SpriteBundle,
    rigid_body: RigidBody,
    collider: Collider,
    linear_velocity: LinearVelocity,
    lifetime: Lifetime,
    layer: CollisionLayers,
    laser: Laser,
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
            laser: Laser,
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
//TODO update this to spawn events so others can listen for them
fn update_lifetimes(
    mut lifetimes_query: Query<(Entity, &mut Lifetime)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let delta = time.delta();
    for (entity, mut lifetime) in lifetimes_query.iter_mut() {
        lifetime.0.tick(delta);
        if lifetime.0.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
#[derive(Component, Debug, Clone, Copy)]
pub enum AsteroidClass {
    Big,
    Medium,
    Small,
    Tiny,
}

/// An event sent for a firing a laser
#[derive(Event)]
pub struct SpawnAsteroidEvent {
    origin: Transform,
    class: AsteroidClass,
    velocity: LinearVelocity,
    angular: AngularVelocity,
}

#[derive(Component)]
pub struct AsteroidHealth(i8);

#[derive(Bundle)]
pub struct AsteroidBundle {
    sprite_bundle: SpriteBundle,
    rigid_body: RigidBody,
    collider: Collider,
    linear_velocity: LinearVelocity,
    layer: CollisionLayers,
    health: AsteroidHealth,
    class: AsteroidClass,
    angular_velocity: AngularVelocity,
}

impl Default for AsteroidBundle {
    fn default() -> Self {
        Self {
            sprite_bundle: Default::default(),
            rigid_body: RigidBody::Dynamic,
            collider: Collider::ball(50.0),
            linear_velocity: Default::default(),
            layer: CollisionLayers::new([Layer::Red], [Layer::Red, Layer::Blue]),
            health: AsteroidHealth(5),
            class: AsteroidClass::Big,
            angular_velocity: AngularVelocity::default(),
        }
    }
}

impl AsteroidBundle {
    pub fn spawn(
        event: &SpawnAsteroidEvent,
        asset_server: &Res<AssetServer>,
        commands: &mut Commands,
    ) {
        let sprite = match event.class {
            AsteroidClass::Big => "meteors/meteorGrey_big1.png",
            AsteroidClass::Medium => "meteors/meteorGrey_med1.png",
            AsteroidClass::Small => "meteors/meteorGrey_small1.png",
            AsteroidClass::Tiny => "meteors/meteorGrey_tiny1.png",
        };
        let scale = match event.class {
            AsteroidClass::Big => 2.0,
            AsteroidClass::Medium => 1.5,
            AsteroidClass::Small => 1.0,
            AsteroidClass::Tiny => 1.0,
        };
        let collider_size = match event.class {
            AsteroidClass::Big => 50.0,
            AsteroidClass::Medium => 22.0,
            AsteroidClass::Small => 15.0,
            AsteroidClass::Tiny => 6.0,
        };
        let health: i8 = match event.class {
            AsteroidClass::Big => 5,
            AsteroidClass::Medium => 4,
            AsteroidClass::Small => 3,
            AsteroidClass::Tiny => 2,
        };

        commands.spawn(AsteroidBundle {
            sprite_bundle: SpriteBundle {
                texture: asset_server.load(sprite),
                transform: event.origin.with_scale(Vec3::splat(scale)),
                ..Default::default()
            },
            collider: Collider::ball(collider_size),
            linear_velocity: event.velocity,
            health: AsteroidHealth(health),
            class: event.class,
            angular_velocity: event.angular,
            ..default()
        });
    }
}

fn asteroid_spawner(
    mut reader: EventReader<SpawnAsteroidEvent>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for event in reader.read() {
        info!("thwomp");
        //spawn asteroid
        AsteroidBundle::spawn(event, &asset_server, &mut commands);
    }
}

pub enum EntityTypes {
    Asteroid,
    Laser,
    Ship,
    Unknown,
}

fn handle_collisions(
    mut events: EventReader<Collision>,
    ships: Query<(Entity, &Ship)>,
    lasers: Query<(Entity, &Laser)>,
    mut asteroids: Query<(Entity, &AsteroidClass, &mut AsteroidHealth)>,
    mut commands: Commands,
) {
    for event in events.read() {
        info!(
            "{:?} and {:?} are colliding",
            event.0.entity1, event.0.entity2
        );

        let asteroid = asteroids.get(event.0.entity1);
        let thing1 = match asteroid {
            Ok(_) => EntityTypes::Asteroid,
            Err(_) => {
                let laser = lasers.get(event.0.entity1);
                match laser {
                    Ok(_) => EntityTypes::Laser,
                    Err(_) => {
                        let ship = ships.get(event.0.entity1);
                        match ship {
                            Ok(_) => EntityTypes::Ship,
                            Err(_) => EntityTypes::Unknown,
                        }
                    }
                }
            }
        };
        let asteroid = asteroids.get(event.0.entity2);
        let thing2 = match asteroid {
            Ok(_) => EntityTypes::Asteroid,
            Err(_) => {
                let laser = lasers.get(event.0.entity2);
                match laser {
                    Ok(_) => EntityTypes::Laser,
                    Err(_) => {
                        let ship = ships.get(event.0.entity2);
                        match ship {
                            Ok(_) => EntityTypes::Ship,
                            Err(_) => EntityTypes::Unknown,
                        }
                    }
                }
            }
        };
        //this is all really ugly
        match (thing1, thing2) {
            (EntityTypes::Asteroid, EntityTypes::Asteroid) => {
                info!("bounce")
            }
            (EntityTypes::Asteroid, EntityTypes::Laser) => {
                //despawn laser and decrement health of asteroid
                commands.entity(event.0.entity2).despawn_recursive();
                let asteroid = asteroids.get_mut(event.0.entity1);
                match asteroid {
                    Ok(mut asteroid) => asteroid.2 .0 -= 1,
                    Err(_) => {}
                }
            }
            (EntityTypes::Asteroid, EntityTypes::Ship) => {}
            (EntityTypes::Laser, EntityTypes::Asteroid) => {
                //despawn laser and decrement health of asteroid
                commands.entity(event.0.entity1).despawn_recursive();
                let asteroid = asteroids.get_mut(event.0.entity2);
                match asteroid {
                    Ok(mut asteroid) => asteroid.2 .0 -= 1,
                    Err(_) => {}
                }
            }
            (EntityTypes::Ship, EntityTypes::Asteroid) => {}
            _ => {}
        }
    }
}

fn handle_destroyed_asteroids(
    asteroids: Query<(
        Entity,
        &AsteroidClass,
        &AsteroidHealth,
        &Transform,
        &LinearVelocity,
    )>,
    mut commands: Commands,
    mut asteroid_event_writer: EventWriter<SpawnAsteroidEvent>,
) {
    let mut rng = rand::thread_rng();
    let speed = 45.0;
    let rot_speed = 5.0;
    for asteroid in asteroids.iter() {
        if asteroid.2 .0 <= 0 {
            commands.entity(asteroid.0).despawn_recursive();
            //spawn the children!
            match asteroid.1 {
                AsteroidClass::Big => {
                    //we have a lot of children to spawn lol
                    //center
                    asteroid_event_writer.send(SpawnAsteroidEvent {
                        origin: Transform {
                            translation: asteroid.3.translation,
                            ..default()
                        },
                        class: AsteroidClass::Medium,
                        velocity: LinearVelocity::default(),
                        angular: AngularVelocity::default(),
                    });
                    let count = 6.0;
                    let step = 2.0 * PI / count;
                    //angle offset
                    let angle_offset = rng.gen_range(0.0..360.0);

                    info!("offset : {}", angle_offset);
                    for n in 1..=6 {
                        //velocity
                        let x = rng.gen_range(-speed..speed);
                        let y = rng.gen_range(-speed..speed);
                        let velocity = Vec2 {
                            x: asteroid.4 .0.x + x,
                            y: asteroid.4 .0.y + y,
                        };
                        let rot = rng.gen_range(-rot_speed..rot_speed);
                        let translation = asteroid.3.translation
                            + Quat::from_rotation_z(angle_offset + n as f32 * step)
                                .mul_vec3(Vec3::Y * 68.0);
                        asteroid_event_writer.send(SpawnAsteroidEvent {
                            origin: Transform {
                                translation: translation,
                                ..default()
                            },
                            class: AsteroidClass::Medium,
                            velocity: LinearVelocity(velocity),
                            angular: AngularVelocity(rot),
                        });
                    }
                }
                AsteroidClass::Medium => {
                    //we have a lot of children to spawn lol
                    let count = 3.0;
                    let step = 2.0 * PI / count;
                    //angle offset
                    let angle_offset = rng.gen_range(0.0..360.0);
                    //velocity

                    for n in 1..=3 {
                        let x = rng.gen_range(-speed..speed);
                        let y = rng.gen_range(-speed..speed);
                        let velocity = Vec2 {
                            x: asteroid.4 .0.x + x,
                            y: asteroid.4 .0.y + y,
                        };
                        let rot = rng.gen_range(-rot_speed..rot_speed);
                        let translation = asteroid.3.translation
                            + Quat::from_rotation_z(angle_offset + n as f32 * step)
                                .mul_vec3(Vec3::Y * 20.0);
                        asteroid_event_writer.send(SpawnAsteroidEvent {
                            origin: Transform {
                                translation: translation,
                                ..default()
                            },
                            class: AsteroidClass::Small,
                            velocity: LinearVelocity(velocity),
                            angular: AngularVelocity(rot),
                        });
                    }
                }
                AsteroidClass::Small => {
                    //we have a lot of children to spawn lol
                    let count = 4.0;
                    let step = 2.0 * PI / count;
                    //angle offset
                    let angle_offset = rng.gen_range(0.0..360.0);

                    for n in 1..=4 {
                        //velocity
                        let x = rng.gen_range(-speed..speed);
                        let y = rng.gen_range(-speed..speed);
                        let velocity = Vec2 {
                            x: asteroid.4 .0.x + x,
                            y: asteroid.4 .0.y + y,
                        };
                        let rot = rng.gen_range(-rot_speed..rot_speed);
                        let translation = asteroid.3.translation
                            + Quat::from_rotation_z(angle_offset + n as f32 * step)
                                .mul_vec3(Vec3::Y * 10.0);
                        asteroid_event_writer.send(SpawnAsteroidEvent {
                            origin: Transform {
                                translation: translation,
                                ..default()
                            },
                            class: AsteroidClass::Tiny,
                            velocity: LinearVelocity(velocity),
                            angular: AngularVelocity(rot),
                        });
                    }
                }
                AsteroidClass::Tiny => {}
            };
        }
    }
}

fn wrapper(mut wrapped_entities_query: Query<&mut Transform, Or<(&Ship, &AsteroidClass)>>) {
    for mut entity in wrapped_entities_query.iter_mut() {
        if entity.translation.y > 360.0 {
            entity.translation.y = -entity.translation.y + 1.0;
        }
        if entity.translation.x > 640.0 {
            entity.translation.x = -entity.translation.x + 1.0;
        }
        if entity.translation.y < -360.0 {
            entity.translation.y = -entity.translation.y - 1.0;
        }
        if entity.translation.x < -640.0 {
            entity.translation.x = -entity.translation.x - 1.0;
        }
    }
}
