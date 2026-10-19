use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
};

use bevy::prelude::*;
use rand::RngExt;

pub const SENSITIVITY: (i32, i32) = (10, 400);
pub const VOICE_BOOST: (i32, i32) = (13, 400);

#[derive(Resource, Clone, Debug)]
pub struct Config {
    pub fullscreen: i32,
    pub sound: i32,
    pub sound_volume: i32,
    pub name: String,
    pub invert_mouse: i32,
    pub mouse_sensitivity: i32,
    pub voice_hold_to_talk: i32,
    pub voice_key: i32,
    pub voice_mb: i32,
    pub voice_min: i32,
    pub voice_boost: i32,
    pub vertical_sync: i32,
    pub player_gender: i32,
    pub player_head: i32,
    pub player_skin_color: i32,
    pub player_hair_color: i32,
    pub player_hair_style: i32,
    pub player_eye_color: i32,
    pub manual_hands: i32,
    pub rosa_colours: i32,
}

impl Config {
    pub fn defaults() -> Self {
        let mut rng = rand::rng();
        Self {
            fullscreen: 0,
            sound: 1,
            sound_volume: 80,
            name: format!("Agent {}", rng.random::<u32>() & 0x7fff_ffff & 0x1ff),
            invert_mouse: 0,
            mouse_sensitivity: 100,
            voice_hold_to_talk: 1,
            voice_key: 25,
            voice_mb: 0,
            voice_min: 1024,
            voice_boost: 100,
            vertical_sync: 1,
            player_gender: rng.random_range(0..2),
            player_head: rng.random_range(0..5),
            player_skin_color: rng.random_range(0..6),
            player_hair_color: rng.random_range(0..12),
            player_hair_style: rng.random_range(0..9),
            player_eye_color: rng.random_range(0..8),
            manual_hands: 0,
            rosa_colours: 1,
        }
    }

    pub fn load(path: &Path) -> Self {
        let mut c = Self::defaults();
        let file = std::fs::read(path).unwrap_or_default();

        c.visit(&mut Reader { file: &file });

        c
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut w = Writer { out: String::new() };
        self.clone().visit(&mut w);
        std::fs::write(path, w.out)
    }

    pub fn sensitivity(&self) -> f32 {
        self.mouse_sensitivity.clamp(SENSITIVITY.0, SENSITIVITY.1) as f32 / 100.0
    }

    pub fn voice_gain(&self) -> f32 {
        self.voice_boost.clamp(VOICE_BOOST.0, VOICE_BOOST.1) as f32 / 100.0
    }

    fn visit(&mut self, v: &mut impl Visitor) {
        v.int("fullscreen=", &mut self.fullscreen);
        v.int("sound=", &mut self.sound);
        v.int("soundvolume=", &mut self.sound_volume);
        v.string("name=", 0x1e, &mut self.name);
        v.int("invertmouse=", &mut self.invert_mouse);
        v.int("mousesensitivity=", &mut self.mouse_sensitivity);
        v.int("voiceholdtotalk=", &mut self.voice_hold_to_talk);
        v.int("voicekey=", &mut self.voice_key);
        v.int("voicemb=", &mut self.voice_mb);
        v.int("voicemin=", &mut self.voice_min);
        v.int("voiceboost=", &mut self.voice_boost);
        v.int("verticalsync=", &mut self.vertical_sync);
        v.int("playergender=", &mut self.player_gender);
        v.int("playerhead=", &mut self.player_head);
        v.int("playerskincolor=", &mut self.player_skin_color);
        v.int("playerhaircolor=", &mut self.player_hair_color);
        v.int("playerhairstyle=", &mut self.player_hair_style);
        v.int("playereyecolor=", &mut self.player_eye_color);
        v.int("manualhands=", &mut self.manual_hands);
        v.int("rosacolours=", &mut self.rosa_colours);
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

pub fn config_dir() -> PathBuf {
    let base = if cfg!(windows) { documents_dir() } else { None }.or_else(home_dir).unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Sub Rosa");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(windows)]
fn documents_dir() -> Option<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHGetSpecialFolderPathW(owner: *mut core::ffi::c_void, path: *mut u16, csidl: i32, create: i32) -> i32;
    }
    const CSIDL_PERSONAL: i32 = 5;
    let mut buf = [0u16; 260];
    let ok = unsafe { SHGetSpecialFolderPathW(std::ptr::null_mut(), buf.as_mut_ptr(), CSIDL_PERSONAL, 0) };
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    (ok != 0 && len > 0).then(|| PathBuf::from(OsString::from_wide(&buf[..len])))
}

#[cfg(not(windows))]
fn documents_dir() -> Option<PathBuf> {
    None
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.txt")
}

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Config::load(&config_path())).add_systems(Last, save_on_exit);
    }
}

fn save_on_exit(mut exits: MessageReader<AppExit>, config: Res<Config>) {
    if exits.read().count() > 0
        && let Err(e) = config.save(&config_path())
    {
        warn!("config.txt not saved: {e}");
    }
}

trait Visitor {
    fn int(&mut self, key: &str, v: &mut i32);
    fn string(&mut self, key: &str, max: usize, v: &mut String);
}

struct Reader<'a> {
    file: &'a [u8],
}

impl Reader<'_> {
    fn find(&self, key: &str) -> Option<usize> {
        let k = key.as_bytes();
        let at = self.file.windows(k.len()).position(|w| w == k).unwrap_or(self.file.len()) + k.len();
        (at < self.file.len()).then_some(at)
    }
}

impl Visitor for Reader<'_> {
    fn int(&mut self, key: &str, v: &mut i32) {
        if let Some(at) = self.find(key) {
            *v = read_int(&self.file[at..]);
        }
    }

    fn string(&mut self, key: &str, max: usize, v: &mut String) {
        if let Some(at) = self.find(key) {
            let line: Vec<u8> = self.file[at..].iter().copied().take_while(|&c| c != b'\n' && c != b'\r').take(max - 1).collect();
            *v = String::from_utf8_lossy(&line).into_owned();
        }
    }
}

fn read_int(text: &[u8]) -> i32 {
    let is_num = |c: &u8| c.is_ascii_digit() || *c == b'-';
    let start = text.iter().position(is_num).unwrap_or(text.len());
    let run: Vec<u8> = text[start..].iter().copied().take_while(is_num).take(255).collect();
    scan_int(&run)
}

fn scan_int(s: &[u8]) -> i32 {
    let (neg, digits) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let v = digits.iter().take_while(|c| c.is_ascii_digit()).fold(0i64, |v, &c| (v * 10 + (c - b'0') as i64).min(i64::from(u32::MAX) + 1));
    let v = if neg { -v } else { v };
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

struct Writer {
    out: String,
}

impl Visitor for Writer {
    fn int(&mut self, key: &str, v: &mut i32) {
        let _ = write!(self.out, "{key}{v}\r\n");
    }

    fn string(&mut self, key: &str, _max: usize, v: &mut String) {
        let _ = write!(self.out, "{key}{v}\r\n");
    }
}