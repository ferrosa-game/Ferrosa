use bevy::prelude::*;

use super::{
    Level,
    camera::GameCamera,
    netgame::{NetEvent, NetGame},
    round_events,
};
use crate::{menu::Screen, net::ClientSocket};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetTick {
    Input,
    Send,
    Receive,
    Events,
    Follow,
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<NetEvent>()
            .insert_resource(Time::<Fixed>::from_seconds(1.0 / 60.0))
            .configure_sets(
                FixedUpdate,
                (NetTick::Input, NetTick::Send, NetTick::Receive, NetTick::Events, NetTick::Follow).chain().run_if(resource_exists::<NetGame>),
            )
            .add_systems(FixedUpdate, (send_packet.in_set(NetTick::Send), receive_packets.in_set(NetTick::Receive), play_events.in_set(NetTick::Events)))
            .add_systems(Update, start_new_round.run_if(resource_exists::<NetGame>));
    }
}

fn send_packet(mut net: ResMut<NetGame>, mut socket: ResMut<ClientSocket>, time: Res<Time<Real>>, camera: Option<Single<&Transform, With<GameCamera>>>) {
    let camera = camera.map_or(Vec3::ZERO, |t| t.translation);

    net.send(&mut socket, camera, time.elapsed().as_millis() as u32);
}

fn receive_packets(mut commands: Commands, mut net: ResMut<NetGame>, mut socket: ResMut<ClientSocket>, time: Res<Time<Real>>, mut next: ResMut<NextState<Screen>>) {
    if !net.receive(&mut socket, time.elapsed().as_millis() as u32) {
        warn!("the server stopped answering");

        commands.remove_resource::<NetGame>();

        next.set(Screen::Main);
    }
}

fn play_events(mut commands: Commands, mut net: ResMut<NetGame>, mut happenings: MessageWriter<NetEvent>) {
    net.process_events();
    happenings.write_batch(net.happenings.drain(..));
    round_events::watch(&mut commands, &mut net);
}

fn start_new_round(mut commands: Commands, mut net: ResMut<NetGame>, level: Option<Res<Level>>, mut next: ResMut<NextState<Screen>>) {
    if net.server_round == net.round {
        return;
    }

    net.reset_round();

    if level.is_some_and(|l| l.map.map_name != net.sync.map) {
        commands.insert_resource(net.sync.clone());
        next.set(Screen::Loading);
    }
}
