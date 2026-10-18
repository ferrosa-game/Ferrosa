use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor, ImageType},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

pub const ATLAS_SIZE: u32 = 0x80;
const WINDOW: [u8; 3] = [0x70, 0x80, 0x90];
const WINDOW_ROUGHNESS: u8 = 0x10;
const YELLOW: [u8; 3] = [0xff, 0xff, 0x20];
const WHITE: [u8; 3] = [0xff; 3];

#[derive(Clone)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Pixels {
    pub fn decode_png(bytes: &[u8]) -> Option<Self> {
        let image = Image::from_buffer(bytes, ImageType::Extension("png"), default(), true, ImageSampler::Default, RenderAssetUsages::RENDER_WORLD).ok()?;
        let (width, height) = (image.width(), image.height());
        let mut rgba = image.convert(TextureFormat::Rgba8UnormSrgb)?.data?;

        rgba.chunks_mut(4).for_each(|p| p[3] = 255);

        Some(Self { width, height, rgba })
    }

    pub fn into_image(self) -> Image {
        material_image(self, TextureFormat::Rgba8UnormSrgb)
    }

    pub fn into_linear_image(self) -> Image {
        material_image(self, TextureFormat::Rgba8Unorm)
    }
}

fn material_image(pixels: Pixels, format: TextureFormat) -> Image {
    let Pixels { width, height, rgba } = pixels;
    let (data, levels) = with_mipmaps(width, height, rgba.clone());
    let mut image = Image::new(Extent3d { width, height, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, format, RenderAssetUsages::RENDER_WORLD);

    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 16,
        ..default()
    });

    image
}

fn with_mipmaps(width: u32, height: u32, rgba: Vec<u8>) -> (Vec<u8>, u32) {
    let mut data = rgba.clone();
    let (mut w, mut h, mut level) = (width, height, rgba);
    let mut count = 1;

    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];

        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let px = |xx: u32, yy: u32| level[((yy.min(h - 1) * w + xx.min(w - 1)) * 4 + c) as usize] as u32;
                    let sum = px(2 * x, 2 * y) + px(2 * x + 1, 2 * y) + px(2 * x, 2 * y + 1) + px(2 * x + 1, 2 * y + 1);
                    next[((y * nw + x) * 4 + c) as usize] = (sum / 4) as u8;
                }
            }
        }

        data.extend_from_slice(&next);
        (w, h, level) = (nw, nh, next);
        count += 1;
    }

    (data, count)
}

fn hash_noise(x: i32, y: i32) -> f32 {
    let n = x.wrapping_add(y.wrapping_mul(0x39));
    let n = (n << 13) ^ n;
    let m = n.wrapping_mul(n).wrapping_mul(0x3d73).wrapping_add(0xc0ae5);
    let k = m.wrapping_mul(m).wrapping_add(0x5208dd0d) & 0x7fffffff;

    1.0 - k as f32 / 1_073_741_824.0
}

pub fn cosine_noise(mask: i32, x: f32, y: f32) -> f32 {
    let lerp = |a: f32, b: f32, t: f32| {
        let f = ((1.0 - (t as f64 * std::f64::consts::PI).cos()) * 0.5) as f32;
        (1.0 - f) * a + f * b
    };

    let (ix, iy) = (x as i32, y as i32);
    let fx = x - ix as f32;
    let (ix1, iy1) = ((ix + 1) & mask, (iy + 1) & mask);
    let top = lerp(hash_noise(ix, iy1), hash_noise(ix1, iy1), fx);

    lerp(lerp(hash_noise(ix, iy), hash_noise(ix1, iy), fx), top, y - iy as f32)
}

pub fn concrete() -> Pixels {
    let rgba = (0..0x80)
        .flat_map(|y: i32| (0..0x80).map(move |x: i32| (x, y)))
        .flat_map(|(x, y)| {
            let (fx, fy) = (x as f32, y as f32);
            let n = |mask: i32, a: f32, b: f32| cosine_noise(mask, a, b) + 1.0;
            let (n1, n2, n3, n4) = (n(0xf, fx * 0.25, fy * 0.125), n(0x1f, fx * 0.5, fy * 0.25), n(0x3f, fx, fy * 0.5), n(0x7f, fx + fx + 1235.0, fy + 123.0));
            let s = n4 + n4 + n3 + n3 + n2 + n2 + n1 + n1;
            let (a, b) = ((s + 160.0) as i32 as u8, (s * 0.875 + 160.0) as i32 as u8);

            [a, a, b, 255]
        })
        .collect();

    Pixels { width: 0x80, height: 0x80, rgba }
}

fn marking(layer: usize, row: i32, col: i32) -> Option<[u8; 3]> {
    let span = |v: i32, lo: i32, len: i32| (v - lo) as u32 <= len as u32;
    let (row_a, row_b, row_mid) = (span(row, 5, 5), span(row, 117, 5), span(row, 32, 63));
    let (col_a, col_b, col_mid) = (span(col, 5, 5), span(col, 117, 5), span(col, 32, 63));

    let yellow = (layer == 1 && row_a) || (layer == 2 && row_b) || (layer == 6 && col_a) || (layer == 7 && col_b);
    let white = (layer == 4 && row_a && col_mid)
        || (layer == 5 && row_b && col_mid)
        || (layer == 9 && row_mid && col_a)
        || (layer == 10 && row_mid && col_b)
        || (layer == 3 && (span(row, 16, 15) || span(row, 96, 15)))
        || (layer == 8 && (span(col, 16, 15) || span(col, 96, 15)));

    if yellow { Some(YELLOW) } else if white { Some(WHITE) } else { None }
}

pub fn particle() -> Pixels {
    let rgba = (0..0x80)
        .flat_map(|i: i32| (0..0x80).map(move |j: i32| (i, j)))
        .flat_map(|(i, j)| {
            let (dx, dy) = ((0x40 - j) as f32, (0x40 - i) as f32);
            let noise = cosine_noise(-1, 0.125 * j as f32 * 0.5, i as f32 * 0.125 * 0.5);
            [0xff, 0xff, 0xff, if noise * 6.0 + 48.0 > (dx * dx + dy * dy).sqrt() { 0xc0 } else { 0 }]
        })
        .collect();

    Pixels { width: 0x80, height: 0x80, rgba }
}

pub fn road_markings(asphalt: &Pixels, layer: usize) -> Pixels {
    let Pixels { width: w, height: h, .. } = *asphalt;
    let mut rgba = asphalt.rgba.clone();

    for y in 0..h {
        for x in 0..w {
            if let Some(c) = marking(layer, 0x7f - (y * 0x80 / h) as i32, (x * 0x80 / w) as i32) {
                rgba[((y * w + x) * 4) as usize..][..3].copy_from_slice(&c);
            }
        }
    }

    Pixels { width: w, height: h, rgba }
}

pub fn window_facade(wall: Option<&Pixels>, [x, w, y, h]: [f32; 4]) -> (Pixels, Pixels) {
    let n = ATLAS_SIZE;
    let inside = |v: u32, lo: f32, len: f32| (lo..lo + len).contains(&((v as f32 + 0.5) / n as f32));
    let (mut colour, mut surface) = (Vec::with_capacity((n * n * 4) as usize), Vec::with_capacity((n * n * 4) as usize));

    for row in 0..n {
        for col in 0..n {
            let glass = inside(col, x, w) && inside(n - 1 - row, y, h);
            let wall = wall.map_or([0x80; 3], |p| {
                let i = (((row * p.height / n) * p.width + col * p.width / n) * 4) as usize;
                [p.rgba[i], p.rgba[i + 1], p.rgba[i + 2]]
            });
            let c = if glass { WINDOW } else { wall };

            colour.extend_from_slice(&[c[0], c[1], c[2], 255]);
            surface.extend_from_slice(&if glass { [0, WINDOW_ROUGHNESS, 255, 255] } else { [0, 230, 0, 255] });
        }
    }

    (Pixels { width: n, height: n, rgba: colour }, Pixels { width: n, height: n, rgba: surface })
}
