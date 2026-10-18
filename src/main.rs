#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

#[cfg(windows)]
use bevy::{ecs::system::NonSendMarker, window::PrimaryWindow};
use bevy::prelude::*;

mod audio;
mod config;
mod fonts;
mod game;
mod human;
mod masterserver;
mod menu;
mod net;
mod steam;
mod ui;
mod rich_presence;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin { primary_window: Some(Window { title: "Ferrosa".into(), ..default() }), ..default() })
                .set(AssetPlugin { file_path: "data".into(), ..default() }),
        )
        .insert_resource(ClearColor(menu::MENU_BACKGROUND))
        .add_plugins((config::ConfigPlugin, fonts::FontsPlugin, audio::DevicesPlugin))
        .add_plugins((
            masterserver::MasterServerPlugin,
            ui::UiPlugin,
            menu::MenuPlugin,
            human::HumanPlugin,
            game::GamePlugin,
            rich_presence::RichPresencePlugin,
        ))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn((Camera2d, IsDefaultUiCamera));
        })
        .add_systems(Update, set_window_icon)
        .run();
}

#[cfg(windows)]
fn set_window_icon(_: NonSendMarker, primary: Single<Entity, With<PrimaryWindow>>, mut done: Local<bool>) {
    use winit::platform::windows::{IconExtWindows, WindowExtWindows};

    if *done {
        return;
    }

    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        let Some(window) = windows.get_window(*primary) else { return };

        if let Ok(icon) = winit::window::Icon::from_resource(1, None) {
            window.set_window_icon(Some(icon.clone()));
            window.set_taskbar_icon(Some(icon))
        }

        *done = true;
    })
}