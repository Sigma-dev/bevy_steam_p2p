use bevy::*;
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

    client.create_lobby();
    
}

fn lobby_joined(
    mut event_reader: EventReader<LobbyJoined>,
    mut client: ResMut<SteamP2PClient>
) {
    for ev in event_reader.read() {
        client.instantiate(FilePath(0),Vec3 {x:1., y:2., z: 1.}).unwrap_or_else(|e| eprintln!("Instantiation error: {e}"));
        client.instantiate(FilePath(0),Vec3 {x:1., y:2., z: 1.}).unwrap_or_else(|e| eprintln!("Instantiation error: {e}"));
    }
}