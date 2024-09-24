use bevy::*;
use client::SteamP2PClient;
use prelude::*;
use bevy_steam_p2p::*;



fn main() {
    App::new()
    .add_plugins(SteamP2PPlugin)
    .add_plugins(DefaultPlugins)
    .add_systems(Startup, startup)
    .add_systems(Update, update)
    .run();
}

fn startup(
    mut commands: Commands,
) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(5., 5., 5.).looking_at(Vec3::new(0.0, 0., 0.0), Vec3::Y),
        ..default()
    });
}

/*
    1. Have one of your clients create a server (PRESS C)
    2. Have the other clients join using the steam friends list
    3. Have fun with replicated objects
*/
fn update(
    mut client: ResMut<SteamP2PClient>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::KeyC) {
        client.create_lobby(8);
    }
    if keys.just_pressed(KeyCode::KeyT) {
        client.send_message_all(NetworkData::DebugMessage("Hello world !".to_owned()), SendFlags::RELIABLE);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        client.instantiate(FilePath::new("InstantiationExample"), Vec3::new(0., 2., 0.));
    }
}