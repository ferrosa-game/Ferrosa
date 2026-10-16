mod config;
mod human;
mod masterserver;
mod menu;
mod net;
mod steam;
mod ui;

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin { primary_window: Some(Window { title: "Ferrosa".into(), ..default() }), ..default() })
                .set(AssetPlugin { file_path: "data".into(), ..default() }),
        )
        .insert_resource(ClearColor(menu::MENU_BACKGROUND))
        .add_plugins(config::ConfigPlugin)
        .add_plugins((
            masterserver::MasterServerPlugin,
            ui::UiPlugin,
            menu::MenuPlugin,
            human::HumanPlugin
        ))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn((Camera2d, IsDefaultUiCamera));
        })
        .add_systems(Update, screenshot_and_quit)
        .run();
}

/// SCREENSHOT=<file.png>: the window saved SCREENSHOT_SECS seconds in (one by default), then the app closes.
fn screenshot_and_quit(mut commands: Commands, time: Res<Time<Real>>, mut taken: Local<Option<f32>>) {
    let Ok(path) = std::env::var("SCREENSHOT") else { return };

    let at = std::env::var("SCREENSHOT_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let now = time.elapsed_secs();

    match *taken {
        None if now >= at => {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            *taken = Some(now);
        }
        Some(t) if now >= t + 1.0 => {
            commands.write_message(AppExit::Success);
        }
        _ => {}
    }
}

