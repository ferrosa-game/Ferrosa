use bevy::{asset::AssetId, prelude::*};

const VCR: &[u8] = include_bytes!("../data/fonts/vcr.ttf");
const OSD_WHITE: &[u8] = include_bytes!("../data/fonts/osd-white.ttf");
const ROCKWELL: &[u8] = include_bytes!("../data/fonts/rockwell.ttf");
const FIXEDSYS: &[u8] = include_bytes!("../data/fonts/fixedsys.ttf");
pub const OVERPASS: &[u8] = include_bytes!("../data/fonts/overpass.ttf");

#[derive(Resource)]
pub struct GameFonts {
    pub osd_white: Handle<Font>,
    pub rockwell: Handle<Font>,
    pub fixedsys: Handle<Font>,
    pub overpass: Handle<Font>,
}

pub struct FontsPlugin;

impl Plugin for FontsPlugin {
    fn build(&self, app: &mut App) {
        let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();

        let _ = fonts.insert(AssetId::default(), Font::from_bytes(VCR.to_vec()));

        let game_fonts = GameFonts {
            osd_white: fonts.add(Font::from_bytes(OSD_WHITE.to_vec())),
            rockwell: fonts.add(Font::from_bytes(ROCKWELL.to_vec())),
            fixedsys: fonts.add(Font::from_bytes(FIXEDSYS.to_vec())),
            overpass: fonts.add(Font::from_bytes(OVERPASS.to_vec())),
        };

        app.insert_resource(game_fonts);
    }
}
