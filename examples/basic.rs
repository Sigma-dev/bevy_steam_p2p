use bevy::prelude::*;
use bevy_steam_p2p::{
    networked_events::{event::Networked, register::NetworkedEvents},
    FilePath, NetworkData,
};
use bevy_steam_p2p::{SteamP2PClient, SteamP2PPlugin};
use serde::{Deserialize, Serialize};
use steamworks::networking_types::SendFlags;

#[derive(Event, Serialize, Deserialize, Clone, Copy)]
struct TestEvent {
    n: u32,
}

fn main() {
    App::new()
        .add_plugins(SteamP2PPlugin)
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, startup)
        .add_systems(Update, (update, listener))
        .add_networked_event::<TestEvent>()
        .run();
}

fn startup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5., 5., 5.).looking_at(Vec3::new(0.0, 0., 0.0), Vec3::Y),
    ));
}

/*
    1. Have one of your clients create a server (PRESS C)
    2. Have the other clients join using the steam friends list
    3. Have fun with replicated objects
*/
fn update(
    mut client: ResMut<SteamP2PClient>,
    keys: Res<ButtonInput<KeyCode>>,
    mut test_w: EventWriter<Networked<TestEvent>>,
) {
    if keys.just_pressed(KeyCode::KeyC) {
        client.create_lobby(8);
    }
    if keys.just_pressed(KeyCode::KeyT) {
        client
            .send_message_all(
                NetworkData::DebugMessage("Hello world !".to_owned()),
                SendFlags::RELIABLE,
            )
            .expect("Couldn't send hello world message");
    }
    if keys.just_pressed(KeyCode::KeyR) {
        client
            .instantiate(
                FilePath::new("InstantiationExample"),
                None,
                Transform::from_translation(Vec3::new(0., 2., 0.)),
            )
            .expect("Couldn't spawn instantiation example");
    }
    if keys.just_pressed(KeyCode::KeyY) {
        test_w.write(Networked::new(TestEvent { n: 42 }));
    }
}

fn listener(mut test_r: EventReader<TestEvent>) {
    for test in test_r.read() {
        println!("Received test event: {}", test.n);
    }
}
