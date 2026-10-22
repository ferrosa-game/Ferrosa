use bevy::{
    clipboard::Clipboard,
    prelude::*,
    render::renderer::RenderAdapterInfo,
    ui_widgets::{Activate, ValueChange},
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode},
};
use crate::{
    audio::AudioDevices,
    config::{Config, config_path},
    keys::{key_name, mouse_button_bit, scancode},
    menu::{Screen, back_button, text},
    ui::{self, Shortcut},
};

const COLUMN_GAP: f32 = 64.0;
const ITEM: Vec2 = Vec2::new(240.0, 32.0);
const HEADING: Color = Color::srgb(0.75, 0.75, 0.75);

type Flag = fn(&mut Config) -> &mut i32;

#[derive(Component)]
pub struct OptionsPage;

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
            .add_systems(Startup, apply_window_settings)
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
    let input = ui::field::number_input(commands, parent, label, value, range, 120.0);
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

    flag(&mut commands, left, "Rosa-like Colours", config, |c| &mut c.rosa_colours);

    heading(&mut commands, left, "Controls");
    let sensitivity = ui::slider::slider_int(&mut commands, left, "Mouse Sensitivity", config.mouse_sensitivity.clamp(10, 400), (10, 400), ITEM.x);
    commands.entity(sensitivity).observe(|v: On<ValueChange<f32>>, mut config: ResMut<Config>| {
        let sensitivity = v.value.round() as i32;
        if config.mouse_sensitivity != sensitivity {
            config.mouse_sensitivity = sensitivity;
            save(&config);
        }
    });
    flag(&mut commands, left, "Invert Mouse", config, |c| &mut c.invert_mouse);
    flag(&mut commands, left, "Use advanced inventory", config, |c| &mut c.manual_hands);

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

    let list = Vec2::new(ITEM.x, 256.0);
    let output = ui::dropdown::dropdown(&mut commands, right, "Audio Output", &devices.output, devices.chosen_output, ITEM, list);
    commands.entity(output).observe(|v: On<ValueChange<usize>>, mut devices: ResMut<AudioDevices>| devices.chosen_output = v.value);
    let input = ui::dropdown::dropdown(&mut commands, right, "Audio Input", &devices.input, devices.chosen_input, ITEM, list);
    commands.entity(input).observe(|v: On<ValueChange<usize>>, mut devices: ResMut<AudioDevices>| devices.chosen_input = v.value);

    heading(&mut commands, right, "Voice");
    let boost = ui::slider::slider_int(&mut commands, right, "Voice Volume", config.voice_boost.clamp(13, 400), (13, 400), ITEM.x);
    commands.entity(boost).observe(|v: On<ValueChange<f32>>, mut config: ResMut<Config>| {
        let boost = v.value.round() as i32;
        if config.voice_boost != boost {
            config.voice_boost = boost;
            save(&config);
        }
    });
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
