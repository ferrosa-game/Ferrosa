use bevy::{
    prelude::*, tasks::{IoTaskPool, Task},
};
use futures_lite::future;

#[derive(Resource)]
pub struct CanUpdate {
    pub url: String,
}

#[derive(Component)]
pub struct UpdateCheckTask(Task<Option<String>>);

pub fn open_url(url: &str) {
    #[cfg(windows)]
    let opened = std::process::Command::new("explorer").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let opened = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let opened = std::process::Command::new("xdg-open").arg(url).spawn();

    if let Err(e) = opened {
        warn!("could not open {url}: {e}");
    }
}

pub fn spawn_update_check(mut commands: Commands) {
    let thread_pool = IoTaskPool::get();

    let task = thread_pool.spawn(async move {
        let client = reqwest::blocking::Client::builder()
            .user_agent("FerrosaServer")
            .build()
            .ok()?;

        let current_hash = env!("GIT_HASH");

        let res: serde_json::Value = client
            .get("https://api.github.com/repos/ferrosa-game/dedicated-server/releases/latest")
            .send()
            .ok()?
            .json()
            .ok()?;

        let latest_tag = res.get("tag_name")?.as_str()?;
        let page = res.get("html_url")?.as_str()?.to_string();

        let compare_url = format!(
            "https://api.github.com/repos/ferrosa-game/dedicated-server/compare/{}...{}",
            current_hash, latest_tag
        );

        let compare: serde_json::Value = client.get(&compare_url).send().ok()?.json().ok()?;

        let status = compare.get("status")?.as_str()?;

        (status == "ahead").then_some(page)
    });

    commands.spawn(UpdateCheckTask(task));
}

pub fn handle_update_task(mut commands: Commands, mut q_task: Query<(Entity, &mut UpdateCheckTask)>) {
    for (entity, mut task) in &mut q_task {
        if let Some(result) = future::block_on(future::poll_once(&mut task.0)) {
            if let Some(url) = result {
                commands.insert_resource(CanUpdate { url });
                println!("!!!!!! UPDATE AVAIAILABLE !!!!!!");
            }

            commands.entity(entity).despawn();
        }
    }
}