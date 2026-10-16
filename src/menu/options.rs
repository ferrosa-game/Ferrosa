use bevy::{
    clipboard::Clipboard,
    prelude::*,
    render::renderer::RenderAdapterInfo,
    ui_widgets::{Activate, ValueChange},
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode},
};
use cpal::traits::{DeviceTrait, HostTrait};

use crate::{
    config::{Config, config_path},
    menu::{Screen, back_button, text},
    ui::{self, Shortcut},
};

const COLUMN_GAP: f32 = 64.0;
const ITEM: Vec2 = Vec2::new(240.0, 32.0);
const HEADING: Color = Color::srgb(0.75, 0.75, 0.75);

type Flag = fn(&mut Config) -> &mut i32;

#[derive(Component)]
pub struct OptionsPage;

#[derive(Resource)]
pub struct AudioDevices {
    output: Vec<String>,
    input: Vec<String>,
    chosen_output: usize,
    chosen_input: usize,
}

#[derive(Resource, Default)]
struct VoiceBinding {
    waiting: bool,
    paused: Vec<Entity>,
}

#[derive(Component)]
struct VoiceKeyButton;

#[derive(Component)]
struct VoiceMinRow;

pub(super) struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoiceBinding>()
            .add_systems(Startup, (apply_window_settings, list_audio_devices))
            .add_systems(OnEnter(Screen::Options), options_screen)
            .add_systems(Update, (bind_voice_key, show_voice_settings).run_if(any_with_component::<OptionsPage>))
            .add_observer(|_: On<Remove<OptionsPage>>, mut binding: ResMut<VoiceBinding>| binding.waiting = false);
    }
}

fn apply_window_settings(config: Res<Config>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in &mut windows {
        window.mode = window_mode(config.fullscreen != 0);
        window.present_mode = present_mode(config.vertical_sync != 0);
    }
}

fn window_mode(fullscreen: bool) -> WindowMode {
    if fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed }
}

fn present_mode(sync: bool) -> PresentMode {
    if sync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync }
}

fn list_audio_devices(mut commands: Commands) {
    let host = cpal::default_host();

    let names = |devices: Option<Vec<cpal::Device>>| -> Vec<String> {
        devices.iter().flatten().filter_map(|d| d.description().ok()).map(|d| device_name(&d)).collect()
    };

    commands.insert_resource(AudioDevices {
        output: names(host.output_devices().ok().map(Iterator::collect)),
        input: names(host.input_devices().ok().map(Iterator::collect)),
        chosen_output: 0,
        chosen_input: 0,
    });
}

fn device_name(d: &cpal::DeviceDescription) -> String {
    d.extended().first().map_or_else(|| d.name().to_string(), Clone::clone)
}

fn save(config: &Config) {
    if let Err(e) = config.save(&config_path()) {
        warn!("config.txt not saved: {e}");
    }
}

fn flag(commands: &mut Commands, parent: Entity, label: &str, config: &Config, setting: Flag) -> Entity {
    let mut copy = config.clone();

    let checkbox = ui::checkbox::checkbox(commands, parent, label, *setting(&mut copy) != 0);
    commands.entity(checkbox).observe(move |v: On<ValueChange<bool>>, mut config: ResMut<Config>| {
        *setting(&mut config) = v.value as i32;
        save(&config);
    });

    checkbox
}

fn number(commands: &mut Commands, parent: Entity, label: &str, value: i32, range: (i32, i32), setting: Flag) {
    let input = ui::number_input(commands, parent, label, value, range, 120.0);
    commands.entity(input).observe(move |v: On<ValueChange<i32>>, mut config: ResMut<Config>| {
        *setting(&mut config) = v.value;
        save(&config);
    });
}

fn heading(commands: &mut Commands, parent: Entity, s: &str) {
    commands.spawn((Text::new(s), TextFont::from_font_size(18.0), TextColor(HEADING), Node { margin: UiRect::top(px(12)), ..default() }, ChildOf(parent)));
}

fn options_screen(mut commands: Commands, config: Res<Config>, devices: Res<AudioDevices>) {
    let p = spawn_options(&mut commands, &config, &devices);

    commands.entity(p).insert(DespawnOnExit(Screen::Options));

    back_button(&mut commands, p);
}

pub fn spawn_options(commands: &mut Commands, config: &Config, devices: &AudioDevices) -> Entity {
    let p = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(8),
                ..default()
            },
            OptionsPage,
        ))
        .id();

    let mut commands = commands.reborrow();

    text(&mut commands, p, "Options", 32.0, Color::WHITE);

    let columns = commands.spawn((Node { column_gap: px(COLUMN_GAP), margin: UiRect::top(px(16)), ..default() }, ChildOf(p))).id();
    let column = |commands: &mut Commands| commands.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(8), ..default() }, ChildOf(columns))).id();
    let (left, right) = (column(&mut commands), column(&mut commands));

    heading(&mut commands, left, "Display");

    let fullscreen = flag(&mut commands, left, "Fullscreen", config, |c| &mut c.fullscreen);
    commands.entity(fullscreen).observe(|v: On<ValueChange<bool>>, mut windows: Query<&mut Window, With<PrimaryWindow>>| {
        for mut window in &mut windows {
            window.mode = window_mode(v.value);
        }
    });

    let sync = flag(&mut commands, left, "V-Sync", config, |c| &mut c.vertical_sync);
    commands.entity(sync).observe(|v: On<ValueChange<bool>>, mut windows: Query<&mut Window, With<PrimaryWindow>>| {
        for mut window in &mut windows {
            window.present_mode = present_mode(v.value);
        }
    });

    flag(&mut commands, left, "Frame Smoothing", config, |c| &mut c.frame_smoothing);
    flag(&mut commands, left, "Sprite Models", config, |c| &mut c.sprite_models);

    heading(&mut commands, left, "Controls");
    flag(&mut commands, left, "Invert Mouse", config, |c| &mut c.invert_mouse);
    flag(&mut commands, left, "Use advanced inventory", config, |c| &mut c.manual_hands);

    heading(&mut commands, left, "Network");
    number(&mut commands, left, "Port", config.port, (0, 0xffff), |c| &mut c.port);
    number(&mut commands, left, "Bandwidth", config.server_max_bytes_per_second, (10000, 1_000_000_000), |c| &mut c.server_max_bytes_per_second);

    let copy = ui::button(&mut commands, left, "Copy System Info", ITEM);
    commands.entity(copy).observe(copy_system_info);

    heading(&mut commands, right, "Sound");
    let volume = ui::slider::slider_int(&mut commands, right, "Sound Volume", config.sound_volume, (0, 100), ITEM.x);
    commands.entity(volume).observe(|v: On<ValueChange<f32>>, mut config: ResMut<Config>| {
        let volume = v.value.round() as i32;
        if config.sound_volume != volume {
            config.sound_volume = volume;
            save(&config);
        }
    });

    // TODO: the chosen devices opened for sound and voice once the client plays them.
    let list = Vec2::new(ITEM.x, 256.0);
    let output = ui::spinner::spinner(&mut commands, right, "Audio Output", &devices.output, devices.chosen_output, ITEM, list);
    commands.entity(output).observe(|v: On<ValueChange<usize>>, mut devices: ResMut<AudioDevices>| devices.chosen_output = v.value);
    let input = ui::spinner::spinner(&mut commands, right, "Audio Input", &devices.input, devices.chosen_input, ITEM, list);
    commands.entity(input).observe(|v: On<ValueChange<usize>>, mut devices: ResMut<AudioDevices>| devices.chosen_input = v.value);

    heading(&mut commands, right, "Voice");
    flag(&mut commands, right, "Use Push to Talk", config, |c| &mut c.voice_hold_to_talk);
    let min_row = commands.spawn((Node::default(), VoiceMinRow, ChildOf(right))).id();
    number(&mut commands, min_row, "Voice Min", config.voice_min, (0x10, 0x4000), |c| &mut c.voice_min);
    let row = commands.spawn((Node { align_items: AlignItems::Center, column_gap: px(6), ..default() }, ChildOf(right))).id();
    let key = ui::button(&mut commands, row, "", ITEM);
    commands.entity(key).insert(VoiceKeyButton).observe(start_binding);
    ui::label(&mut commands, row, "Voice Key", ui::TEXT_SIZE, Color::WHITE);

    p
}

fn start_binding(_: On<Activate>, mut binding: ResMut<VoiceBinding>, shortcuts: Query<(Entity, &Shortcut)>, mut commands: Commands) {
    if binding.waiting {
        return;
    }
    
    binding.waiting = true;
    binding.paused = shortcuts.iter().filter(|(_, s)| s.0 == KeyCode::Escape).map(|(e, _)| e).collect();
    for &e in &binding.paused {
        commands.entity(e).remove::<Shortcut>();
    }
}

fn bind_voice_key(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut binding: ResMut<VoiceBinding>,
    mut config: ResMut<Config>,
    mut commands: Commands,
) {
    if !binding.waiting {
        return;
    }

    let key = keys.get_just_released().filter_map(|&k| scancode(k)).find(|&s| !key_name(s).is_empty());
    if let Some(s) = key {
        config.voice_key = s;
    }

    let button = mouse.get_just_pressed().next().copied();

    if let Some(bit) = button.and_then(mouse_button_bit).filter(|&bit| bit > 2) {
        config.voice_mb = 1 << bit;
        config.voice_key = 0;
    }

    if key.is_some() || button.is_some() {
        save(&config);

        binding.waiting = false;

        for e in std::mem::take(&mut binding.paused) {
            commands.entity(e).insert(Shortcut(KeyCode::Escape));
        }
    }
}

fn show_voice_settings(
    config: Res<Config>,
    binding: Res<VoiceBinding>,
    buttons: Query<&Children, With<VoiceKeyButton>>,
    mut texts: Query<&mut Text>,
    mut rows: Query<&mut Node, With<VoiceMinRow>>,
) {
    let label = if binding.waiting {
        "Press a key or mouse button".to_string()
    } else if config.voice_key != 0 {
        key_name(config.voice_key).to_string()
    } else if config.voice_mb != 0 {
        format!("Mouse {}", config.voice_mb.trailing_zeros() + 1)
    } else {
        String::new()
    };
    for children in &buttons {
        for &child in children {
            if let Ok(mut text) = texts.get_mut(child)
                && text.0 != label
            {
                text.0.clone_from(&label);
            }
        }
    }
    let display = if config.voice_hold_to_talk == 0 { Display::Flex } else { Display::None };
    for mut node in &mut rows {
        if node.display != display {
            node.display = display;
        }
    }
}

fn mouse_button_bit(button: MouseButton) -> Option<u32> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Middle => Some(1),
        MouseButton::Right => Some(2),
        MouseButton::Back => Some(3),
        MouseButton::Forward => Some(4),
        MouseButton::Other(n) if n < 16 => Some(n as u32),
        MouseButton::Other(_) => None,
    }
}

fn copy_system_info(_: On<Activate>, adapter: Option<Res<RenderAdapterInfo>>, mut clipboard: ResMut<Clipboard>) {
    let os = sysinfo::System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string());

    let mut info = format!("{os} 64-bit\n\n");

    if let Some(a) = adapter {
        info += &format!("Adapter: {}\nDriver: {} {}\nBackend: {:?}\n\n", a.name, a.driver, a.driver_info, a.backend);
    }

    let system = sysinfo::System::new_with_specifics(sysinfo::RefreshKind::nothing().with_memory(sysinfo::MemoryRefreshKind::everything()));
    info += &format!("RAM: {} MB\n", system.total_memory() / (1024 * 1024));

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if std::arch::is_x86_feature_detected!("sse2") {
        info += "SSE2\n";
    }

    if let Err(e) = clipboard.set_text(info) {
        warn!("system info not copied: {e:?}");
    }
}

fn scancode(key: KeyCode) -> Option<i32> {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI,
        KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR,
        KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
    ];

    const DIGITS: [KeyCode; 10] = [
        KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8,
        KeyCode::Digit9, KeyCode::Digit0,
    ];

    if let Some(k) = LETTERS.iter().position(|&l| l == key) {
        return Some(4 + k as i32);
    }

    if let Some(k) = DIGITS.iter().position(|&d| d == key) {
        return Some(30 + k as i32);
    }

    Some(match key {
        KeyCode::Enter => 40,
        KeyCode::Space => 44,
        KeyCode::Comma => 54,
        KeyCode::Period => 55,
        KeyCode::ArrowRight => 79,
        KeyCode::ArrowLeft => 80,
        KeyCode::ArrowDown => 81,
        KeyCode::ArrowUp => 82,
        KeyCode::Numpad7 => 95,
        KeyCode::ControlLeft => 224,
        KeyCode::ShiftLeft => 225,
        KeyCode::AltLeft => 226,
        _ => return None,
    })
}

fn key_name(scancode: i32) -> &'static str {
    const LETTERS: [&str; 26] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z"];
    const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

    match scancode {
        0 => "None",
        4..=29 => LETTERS[(scancode - 4) as usize],
        30..=39 => DIGITS[(scancode - 30) as usize],
        40 => "Enter",
        44 => "Space",
        54 => "Comma",
        55 => "Period",
        79 => "Right",
        80 => "Left",
        81 => "Down",
        82 => "Up",
        95 => "Home",
        224 => "Ctrl",
        225 => "Shift",
        226 => "Alt",
        256 => "Ins",
        257 => "End",
        259 => "PgDn",
        261 => "Center",
        265 => "PgUp",
        266 => "Del",
        269 => "Minus",
        270 => "Plus",
        _ => "",
    }
}