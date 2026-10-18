use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use bevy::{
    asset::RenderAssetUsages,
    gltf::GltfMaterialName,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use ferrosa_protocol::clientbound::game::ItemKind;

use super::{
    items::NetItem,
    netgame::{entities::Item, NetGame},
};
use crate::{
    fonts::{FIXEDSYS, ROCKWELL},
    menu::Screen,
};

const SCREEN_SIZE: u32 = 0x200;
const PAGE_SIZE: u32 = 0x400;
const GREY: [u8; 3] = [0x80; 3];
const BLACK: [u8; 3] = [0; 3];
const SCREEN_BOTTOM: u32 = 384;
const SCREEN_LINES: usize = 0x20;
const SHOWN_LINES: usize = 0x17;
const SCREEN_TEXT: f32 = 16.0;
const PAGE_TEXT: f32 = 32.0;
const HEADLINE: f32 = 48.0;
const MEMO_SIZE: usize = 0x400;
const MATERIAL: &str = "texture";
const PALETTE: [[f32; 3]; 16] = [
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.667],
    [0.0, 0.667, 0.0],
    [0.0, 0.667, 0.667],
    [0.667, 0.0, 0.0],
    [0.667, 0.0, 0.667],
    [0.667, 0.333, 0.0],
    [0.667, 0.667, 0.667],
    [0.333, 0.333, 0.333],
    [0.333, 0.333, 1.0],
    [0.333, 1.0, 0.333],
    [0.333, 1.0, 1.0],
    [1.0, 0.333, 0.333],
    [1.0, 0.333, 1.0],
    [1.0, 1.0, 0.333],
    [1.0, 1.0, 1.0],
];
const ARCADE_GLYPHS: [[u8; 8]; 40] = [
    [0x00, 0x1c, 0x26, 0x63, 0x63, 0x63, 0x32, 0x1c],
    [0x00, 0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7e],
    [0x00, 0x3e, 0x63, 0x07, 0x1e, 0x3c, 0x70, 0x7f],
    [0x00, 0x3f, 0x06, 0x0c, 0x1e, 0x03, 0x63, 0x3e],
    [0x00, 0x0e, 0x1e, 0x36, 0x66, 0x7f, 0x06, 0x06],
    [0x00, 0x7e, 0x60, 0x7e, 0x03, 0x03, 0x63, 0x3e],
    [0x00, 0x1e, 0x30, 0x60, 0x7e, 0x63, 0x63, 0x3e],
    [0x00, 0x7f, 0x63, 0x06, 0x0c, 0x18, 0x18, 0x18],
    [0x00, 0x3c, 0x62, 0x72, 0x3c, 0x4f, 0x43, 0x3e],
    [0x00, 0x3e, 0x63, 0x63, 0x3f, 0x03, 0x06, 0x3c],
    [0x00, 0x5a, 0x3c, 0x18, 0x18, 0x3c, 0x5a, 0x00],
    [0x00, 0x00, 0x00, 0x7e, 0x00, 0x7e, 0x00, 0x00],
    [0x00, 0x38, 0x44, 0x08, 0x10, 0x10, 0x00, 0x10],
    [0x00, 0x1c, 0x36, 0x63, 0x63, 0x7f, 0x63, 0x63],
    [0x00, 0x7e, 0x63, 0x63, 0x7e, 0x63, 0x63, 0x7e],
    [0x00, 0x1e, 0x33, 0x60, 0x60, 0x60, 0x33, 0x1e],
    [0x00, 0x7c, 0x66, 0x63, 0x63, 0x63, 0x66, 0x7c],
    [0x00, 0x7e, 0x60, 0x60, 0x7c, 0x60, 0x60, 0x7f],
    [0x00, 0x7f, 0x60, 0x60, 0x7e, 0x60, 0x60, 0x60],
    [0x00, 0x1f, 0x30, 0x60, 0x67, 0x63, 0x33, 0x1f],
    [0x00, 0x63, 0x63, 0x63, 0x7f, 0x63, 0x63, 0x63],
    [0x00, 0x7e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7e],
    [0x00, 0x03, 0x03, 0x03, 0x03, 0x03, 0x63, 0x3e],
    [0x00, 0x63, 0x66, 0x6c, 0x78, 0x7c, 0x6e, 0x67],
    [0x00, 0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7f],
    [0x00, 0x63, 0x77, 0x7f, 0x7f, 0x6b, 0x63, 0x63],
    [0x00, 0x63, 0x73, 0x7b, 0x7f, 0x6f, 0x67, 0x63],
    [0x00, 0x3e, 0x63, 0x63, 0x63, 0x63, 0x63, 0x3e],
    [0x00, 0x7e, 0x63, 0x63, 0x63, 0x7e, 0x60, 0x60],
    [0x00, 0x3e, 0x63, 0x63, 0x63, 0x6f, 0x66, 0x3d],
    [0x00, 0x7e, 0x63, 0x63, 0x67, 0x7c, 0x6e, 0x67],
    [0x00, 0x3c, 0x66, 0x60, 0x3e, 0x03, 0x63, 0x3e],
    [0x00, 0x7e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18],
    [0x00, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x3e],
    [0x00, 0x63, 0x63, 0x63, 0x77, 0x3e, 0x1c, 0x08],
    [0x00, 0x63, 0x63, 0x6b, 0x7f, 0x7f, 0x77, 0x63],
    [0x00, 0x63, 0x77, 0x3e, 0x1c, 0x3e, 0x77, 0x63],
    [0x00, 0x66, 0x66, 0x66, 0x3c, 0x18, 0x18, 0x18],
    [0x00, 0x7e, 0x7e, 0x0c, 0x18, 0x30, 0x7e, 0x7e],
    [0x00; 8],
];
const BLANK_GLYPH: usize = 0x27;
const FIRST_LETTER: usize = 0xd;
const ARCADE_CELL: u32 = 16;
const CHOMPER: u32 = 32;
const MOUTH: f64 = 0.589_048_622_548_086_2;

#[derive(Component)]
struct ScreenItem {
    version: Option<u32>,
    image: Option<Handle<Image>>,
    material: Option<Handle<StandardMaterial>>,
}

#[derive(Component)]
struct ScreenSlot(Handle<StandardMaterial>);

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (add_screen_items, mark_slots, draw_screens, dress_slots).chain().run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>));
    }
}

fn has_screen(kind: ItemKind) -> bool {
    matches!(kind, ItemKind::Computer | ItemKind::Arcade | ItemKind::Paper | ItemKind::PaperWorld)
}

struct Canvas {
    size: u32,
    rgba: Vec<u8>,
}

impl Canvas {
    fn new(size: u32) -> Self {
        Self { size, rgba: (0..size * size).flat_map(|_| [GREY[0], GREY[1], GREY[2], 255]).collect() }
    }

    fn put(&mut self, x: i32, y: i32, c: [u8; 3], cover: f32) {
        if x < 0 || y < 0 || x >= self.size as i32 || y >= self.size as i32 {
            return;
        }

        let i = ((y as u32 * self.size + x as u32) * 4) as usize;

        for k in 0..3 {
            self.rgba[i + k] = (self.rgba[i + k] as f32 + (c[k] as f32 - self.rgba[i + k] as f32) * cover.clamp(0.0, 1.0)) as u8;
        }
    }

    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        for j in y..y + h {
            for i in x..x + w {
                self.put(i, j, c, 1.0);
            }
        }
    }

    fn glyph(&mut self, font: &FontRef, c: char, x: f32, top: f32, size: f32, colour: [u8; 3]) -> f32 {
        let scaled = font.as_scaled(PxScale::from(size));
        let id = scaled.glyph_id(c);
        let glyph = id.with_scale_and_position(scaled.scale(), point(x, top + scaled.ascent()));

        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, cover| self.put(bounds.min.x as i32 + gx as i32, bounds.min.y as i32 + gy as i32, colour, cover));
        }

        scaled.h_advance(id)
    }

    fn text(&mut self, font: &FontRef, s: &[u8], x: f32, top: f32, size: f32, colour: [u8; 3], cell: Option<f32>) -> f32 {
        let mut at = x;

        for &b in s {
            let advance = self.glyph(font, char::from(b), at, top, size, colour);
            at += cell.unwrap_or(advance);
        }

        at - x
    }

    fn into_image(self) -> Image {
        let mut image = Image::new(Extent3d { width: self.size, height: self.size, depth_or_array_layers: 1 }, TextureDimension::D2, self.rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
        image.sampler = ImageSampler::linear();
        image
    }
}

fn shade(code: u8) -> [u8; 3] {
    PALETTE[(code & 0xf) as usize].map(|c| (c * 255.0) as u8)
}

fn memo_lines(memo: &[u8]) -> Vec<&[u8]> {
    let end = memo.iter().take(MEMO_SIZE).position(|&c| c == 0).unwrap_or(memo.len().min(MEMO_SIZE));
    let text = &memo[..end];

    if text.is_empty() {
        return Vec::new();
    }

    let mut lines: Vec<&[u8]> = text.split(|&c| c == b'\n').collect();

    if text.ends_with(b"\n") {
        lines.pop();
    }

    lines
}

fn computer(item: &Item, fixed: &FontRef) -> Canvas {
    let mut c = Canvas::new(SCREEN_SIZE);
    let cell = SCREEN_TEXT * 0.5;

    c.fill(0, 0, SCREEN_SIZE as i32, SCREEN_BOTTOM as i32, BLACK);

    let mut line = item.top_line as usize & (SCREEN_LINES - 1);

    for k in 0..SHOWN_LINES {
        let y = SCREEN_TEXT + k as f32 * SCREEN_TEXT;

        if let Some((text, colours)) = item.screen.get(line) {
            for (n, &b) in text.iter().take_while(|&&b| b != 0).enumerate() {
                let code = colours.get(n).copied().unwrap_or(0);
                let x = 8.0 + n as f32 * cell;

                c.fill(x as i32, y as i32, cell as i32, SCREEN_TEXT as i32, shade(code >> 4));
                c.glyph(fixed, char::from(b), x, y, SCREEN_TEXT, shade(code));
            }
        }

        line = (line + 1) & (SCREEN_LINES - 1);
    }

    if item.cursor != -1 {
        let x = (item.cursor & 0x3f) as f32 * cell + 8.0;
        let y = (((item.cursor >> 6) - item.top_line) & 0x1f) as f32 * SCREEN_TEXT + SCREEN_TEXT;
        c.fill(x as i32, y as i32, cell as i32, SCREEN_TEXT as i32, shade(0xf));
    }

    c
}

fn memo(item: &Item, fixed: &FontRef) -> Canvas {
    let mut c = Canvas::new(PAGE_SIZE);

    for (k, line) in memo_lines(&item.memo).into_iter().enumerate() {
        c.text(fixed, line, 8.0, 16.0 + PAGE_TEXT * k as f32, PAGE_TEXT, BLACK, Some(PAGE_TEXT * 0.5));
    }

    c
}

fn newspaper(item: &Item, rockwell: &FontRef) -> Canvas {
    let mut c = Canvas::new(PAGE_SIZE);
    let title = b"The Readme";
    let scaled = rockwell.as_scaled(PxScale::from(HEADLINE));
    let width: f32 = title.iter().map(|&b| scaled.h_advance(scaled.glyph_id(char::from(b)))).sum();

    c.text(rockwell, title, (PAGE_SIZE as f32 - width) * 0.5, 16.0, HEADLINE, BLACK, None);

    for (k, line) in memo_lines(&item.memo).into_iter().enumerate() {
        c.text(rockwell, line, 8.0, 96.0 + PAGE_TEXT * k as f32, PAGE_TEXT, BLACK, None);
    }

    c
}

fn chomper(c: &mut Canvas, x: i32, y: i32) {
    let scale = (CHOMPER / 16) as i32;

    for i in 0..16 {
        for j in 0..16 {
            let dy = i as f32 - 7.5;
            let r = ((j as f32 - 7.5).powi(2) + dy * dy).sqrt();
            let a = (dy as f64).atan2((j as f32 - 5.0) as f64);

            if r <= 6.0 && !(-MOUTH < a && a < MOUTH) {
                c.fill(x + j * scale, y + i * scale, scale, scale, [0xff; 3]);
            }
        }
    }
}

fn arcade(item: &Item) -> Canvas {
    let mut c = Canvas::new(SCREEN_SIZE);
    let scale = (ARCADE_CELL / 8) as i32;

    c.fill(0, 0, SCREEN_SIZE as i32, SCREEN_BOTTOM as i32, BLACK);

    for (n, &b) in b"HELLO WORLD".iter().enumerate() {
        let glyph = if b.is_ascii_uppercase() { (b - b'A') as usize + FIRST_LETTER } else { BLANK_GLYPH };
        let x = n as i32 * ARCADE_CELL as i32 + 16;

        for (row, bits) in ARCADE_GLYPHS[glyph].iter().enumerate() {
            for k in 0..8 {
                if bits >> (7 - k) & 1 != 0 {
                    c.fill(x + k * scale, 16 + row as i32 * scale, scale, scale, [0xff; 3]);
                }
            }
        }
    }

    chomper(&mut c, 320, 16);
    chomper(&mut c, item.current_line * 2, item.top_line * 2);

    c
}

fn add_screen_items(mut commands: Commands, items: Query<(Entity, &NetItem), Added<NetItem>>) {
    for (e, item) in &items {
        if has_screen(item.kind) {
            commands.entity(e).insert(ScreenItem { version: None, image: None, material: None });
        }
    }
}

fn mark_slots(mut commands: Commands, meshes: Query<(Entity, &GltfMaterialName, &MeshMaterial3d<StandardMaterial>), Added<GltfMaterialName>>, screens: Query<(), With<ScreenItem>>, parents: Query<&ChildOf>) {
    for (e, name, material) in &meshes {
        if name.0 == MATERIAL && parents.iter_ancestors(e).any(|a| screens.contains(a)) {
            commands.entity(e).insert(ScreenSlot(material.0.clone()));
        }
    }
}

fn draw_screens(net: Res<NetGame>, mut screens: Query<(&NetItem, &mut ScreenItem)>, mut images: ResMut<Assets<Image>>) {
    let (Ok(fixed), Ok(rockwell)) = (FontRef::try_from_slice(FIXEDSYS), FontRef::try_from_slice(ROCKWELL)) else { return };

    for (net_item, mut screen) in &mut screens {
        let Some(item) = net.items.get(net_item.index) else { continue };

        if screen.version == Some(item.text_version) {
            continue;
        }

        let canvas = match net_item.kind {
            ItemKind::Computer => computer(item, &fixed),
            ItemKind::Arcade => arcade(item),
            ItemKind::Paper => memo(item, &fixed),
            _ => newspaper(item, &rockwell),
        };

        match &screen.image {
            Some(handle) => {
                let _ = images.insert(handle, canvas.into_image());
            }
            None => screen.image = Some(images.add(canvas.into_image())),
        }

        screen.version = Some(item.text_version);
    }
}

fn dress_slots(
    mut slots: Query<(Entity, &ScreenSlot, &mut MeshMaterial3d<StandardMaterial>)>,
    mut screens: Query<&mut ScreenItem>,
    parents: Query<&ChildOf>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, slot, mut material) in &mut slots {
        let Some(owner) = parents.iter_ancestors(e).find(|a| screens.contains(*a)) else { continue };
        let Ok(mut screen) = screens.get_mut(owner) else { continue };
        let Some(image) = screen.image.clone() else { continue };

        if screen.material.is_none() {
            let Some(mut m) = materials.get(&slot.0).cloned() else { continue };
            m.base_color = Color::WHITE;
            m.base_color_texture = Some(image);
            screen.material = Some(materials.add(m));
        }

        if let Some(wanted) = screen.material.as_ref().filter(|w| **w != material.0) {
            material.0 = wanted.clone();
        }
    }
}
