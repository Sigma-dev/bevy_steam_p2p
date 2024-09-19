use bevy::*;
use client::SteamP2PClient;
use prelude::*;
use bevy_steam_p2p::*;

fn main() {
    App::new()
    .add_plugins(SteamP2PPlugin)
    .add_plugins(DefaultPlugins)
    .add_systems(Startup, startup)
    .add_systems(Update, lobby_joined)
    .run();
}

fn startup(
    mut commands: Commands,
    mut client: ResMut<SteamP2PClient>
) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(5., 5., 5.).looking_at(Vec3::new(0.0, 0., 0.0), Vec3::Y),
        ..default()
    });

    client.create_lobby(8);
}

fn lobby_joined(
    mut event_reader: EventReader<LobbyJoined>,
    mut client: ResMut<SteamP2PClient>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::KeyT) {
        client.send_message_all(NetworkData::DebugMessage("Hello world !".to_owned()), SendFlags::RELIABLE);
    }
}