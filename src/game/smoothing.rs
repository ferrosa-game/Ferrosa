use std::collections::VecDeque;

use bevy::{ecs::component::Mutable, prelude::*};

use super::{netgame::NetGame, network::TICK_SECONDS};
use crate::{
    character_model::{BONES, HumanPose},
    menu::Screen,
};

const DELAY: f64 = 2.0;
const DRIFT: f64 = 16.0;
const CATCH_UP: f64 = 2.0;
const KEPT: usize = 8;
const FURTHEST: f32 = 32.0;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Smoothing {
    Clock,
    Sync,
    Glide,
    View,
}

#[derive(Resource, Default)]
pub struct RenderTick(Option<f64>);

pub trait Blend: Clone + Send + Sync + 'static {
    fn blend(&self, to: &Self, t: f32) -> Self;

    fn far(&self, _to: &Self) -> bool {
        false
    }
}

impl Blend for Transform {
    fn blend(&self, to: &Self, t: f32) -> Self {
        Transform::from_translation(self.translation.lerp(to.translation, t)).with_rotation(self.rotation.slerp(to.rotation, t))
    }

    fn far(&self, to: &Self) -> bool {
        self.translation.distance(to.translation) > FURTHEST
    }
}

impl Blend for HumanPose {
    fn blend(&self, to: &Self, t: f32) -> Self {
        HumanPose(std::array::from_fn::<_, BONES, _>(|j| self.0[j].slerp(to.0[j], t)))
    }
}

#[derive(Component)]
pub struct Snapshots<T: Blend>(VecDeque<(i32, T)>);

impl<T: Blend> Snapshots<T> {
    pub fn new(tick: i32, value: T) -> Self {
        Self(VecDeque::from([(tick, value)]))
    }

    pub fn last_tick(&self) -> Option<i32> {
        self.0.back().map(|&(t, _)| t)
    }

    pub fn push(&mut self, tick: i32, value: T) {
        match self.0.back() {
            Some(&(last, _)) if tick == last => {
                self.0.pop_back();
            }
            Some((last, place)) if tick < *last || place.far(&value) => self.0.clear(),
            _ => {}
        }

        self.0.push_back((tick, value));

        if self.0.len() > KEPT {
            self.0.pop_front();
        }
    }

    fn at(&self, tick: f64) -> Option<T> {
        match self.0.iter().position(|&(t, _)| t as f64 > tick) {
            Some(0) => self.0.front().map(|(_, v)| v.clone()),
            Some(k) => {
                let ((t0, a), (t1, b)) = (&self.0[k - 1], &self.0[k]);
                Some(a.blend(b, ((tick - *t0 as f64) / (*t1 - *t0) as f64) as f32))
            }
            None => self.0.back().map(|(_, v)| v.clone()),
        }
    }
}

pub struct SmoothingPlugin;

impl Plugin for SmoothingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderTick>()
            .configure_sets(Update, (Smoothing::Clock, Smoothing::Sync, Smoothing::Glide, Smoothing::View).chain().run_if(in_state(Screen::InGame)))
            .add_systems(Update, move_clock.in_set(Smoothing::Clock))
            .add_systems(Update, (glide::<Transform>, glide::<HumanPose>).in_set(Smoothing::Glide))
            .add_systems(OnExit(Screen::InGame), |mut clock: ResMut<RenderTick>| clock.0 = None);
    }
}

fn move_clock(time: Res<Time>, net: Option<Res<NetGame>>, mut clock: ResMut<RenderTick>) {
    let Some(net) = net else { return };
    let target = net.tick as f64 - DELAY;
    let moved = clock.0.map(|tick| tick + time.delta_secs_f64() / TICK_SECONDS);

    clock.0 = Some(match moved {
        Some(tick) if (target - tick).abs() < DRIFT => tick + (target - tick) * (CATCH_UP * time.delta_secs_f64()).min(1.0),
        _ => target,
    });
}

pub fn glide<T: Blend + Component<Mutability = Mutable> + PartialEq>(clock: Res<RenderTick>, mut gliding: Query<(&Snapshots<T>, &mut T)>) {
    let Some(tick) = clock.0 else { return };

    for (snapshots, mut value) in &mut gliding {
        if let Some(v) = snapshots.at(tick) {
            value.set_if_neq(v);
        }
    }
}
