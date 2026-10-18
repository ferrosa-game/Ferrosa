mod admin;
mod blocks;
mod briefing;
mod bullets;
mod camera;
mod chat;
mod controls;
mod doors;
mod effects;
mod eliminator;
mod engines;
mod escape_menu;
mod first_person;
mod hud;
mod human_sync;
mod inventory;
mod items;
mod mute;
mod level_render;
mod loading;
pub mod netgame;
mod network;
mod paper_doll;
mod player_menu;
mod roster;
pub mod round_events;
mod item_displays;
mod sky;
mod smoothing;
mod spectating;
mod street_objects;
mod sounds;
mod terrain;
mod textures;
mod vehicles;
mod voice;

use bevy::prelude::*;
use ferrosa_world::map::Map;

use crate::menu::Screen;

const GAME_FPS: f32 = 60.0;
const GAME_SCREEN: Vec2 = Vec2::new(1024.0, 576.0);

#[derive(Resource)]
pub struct Level {
    pub map: Map,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            loading::LoadingPlugin,
            level_render::LevelPlugin,
            sky::SkyPlugin,
            camera::CameraPlugin,
            controls::ControlsPlugin,
            escape_menu::EscapeMenuPlugin,
            player_menu::PlayerMenuPlugin,
            roster::RosterPlugin,
            first_person::FirstPersonPlugin,
            network::NetworkPlugin,
            human_sync::HumansPlugin,
            vehicles::VehiclesPlugin,
            items::ItemsPlugin,
            smoothing::SmoothingPlugin,
            (
                hud::HudPlugin,
                inventory::InventoryPlugin,
                paper_doll::PaperDollPlugin,
                doors::DoorsPlugin,
                sounds::SoundsPlugin,
                chat::ChatPlugin,
                voice::VoicePlugin,
                spectating::SpectatingPlugin,
                bullets::BulletsPlugin,
                street_objects::StreetObjectsPlugin,
                effects::EffectsPlugin,
                engines::EnginesPlugin,
                mute::MutePlugin,
                briefing::BriefingPlugin
            ),
        ))
        .add_plugins((eliminator::EliminatorPlugin, admin::AdminPlugin, item_displays::ScreensPlugin))
        .add_systems(Update, scale_ui.run_if(in_state(Screen::InGame)))
        .add_systems(OnExit(Screen::InGame), |mut scale: ResMut<UiScale>| scale.0 = 1.0);
    }
}

fn scale_ui(window: Single<&Window>, mut scale: ResMut<UiScale>) {
    let fit = (window.width() / GAME_SCREEN.x).min(window.height() / GAME_SCREEN.y);

    if scale.0 != fit {
        scale.0 = fit;
    }
}
