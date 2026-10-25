use bevy::prelude::*;
use ferrosa_protocol::clientbound::game::{ItemKind, VehicleKind, events::{ServerEvent, Event, chat::ChatType}};

use crate::game::netgame::{CHAT_LENGTH, CellChange, CHAT_LINES, ChatLine, FRESH, ITEMS, LONG_CHAT, NetEvent, NetGame, PLAYERS, PROTECTOR, RESULT, ROLE, SHORT_CHAT, TARGET, entities::Track};

impl NetGame {
    pub fn process_events(&mut self) {
        while self.events_played != self.events_received {
            if let Some(e) = self.events[self.events_played as usize].take() {
                self.event(e);
            }

            self.events_played = (self.events_played + 1) & 0xffff;
        }
    }

    fn happening(&self, kind: &ServerEvent) -> Option<NetEvent> {
        Some(match kind {
            ServerEvent::Bullet(e) => NetEvent::Gunshot { kind: self.items.get(e.item_id as usize).and_then(|i| i.kind), item: e.item_id, bullet: e.bullet_type, pos: e.pos, vel: e.vel },
            ServerEvent::BulletHit(e) => NetEvent::BulletHit { hit: e.hit_type, pos: e.pos, normal: e.normal },
            ServerEvent::BulletHole(e) => NetEvent::BulletHole { pos: e.pos, vel: e.vel, cell: IVec3::new(e.block_x, e.block_y, e.block_z), face: e.face },
            ServerEvent::Explosion(e) => NetEvent::Explosion { pos: e.pos },
            ServerEvent::UpdateVehicle(e) if e.kind == 0 && self.vehicles.get(e.vehicle_id as usize).is_some_and(|v| v.kind != VehicleKind::Train) => {
                NetEvent::WindowBroke { vehicle: e.vehicle_id, window: e.part, pos: e.pos, vel: e.velocity }
            }
            ServerEvent::Sound(e) => NetEvent::Sound { sound: e.sound_type, pos: e.pos, volume: e.volume, pitch: e.pitch },
            ServerEvent::PhoneSound(e) => {
                let item = self.items.get(e.item_id as usize).filter(|i| i.active)?;
                NetEvent::Sound { sound: e.sound, pos: item.track.pos, volume: e.volume, pitch: e.pitch }
            }
            _ => return None,
        })
    }

    fn event(&mut self, ev: Event) {
        let fresh = self.tick.wrapping_sub(ev.tick_created as i32) as u32 <= FRESH;

        if let Some(happening) = self.happening(&ev.kind).filter(|_| fresh) {
            self.happenings.push(happening);
        }

        match ev.kind {
            ServerEvent::Chat(event) => {
                let muted = event.chat_type == ChatType::Announce && usize::try_from(event.speaker_id).ok().and_then(|p| self.players.get(p)).is_some_and(|p| p.muted);
                let spectator = event.chat_type == ChatType::Announce && event.speaker_id >= 0;

                if !muted && !(spectator && self.alive()) && (fresh || matches!(event.chat_type, ChatType::EliminatorAnnouncement | ChatType::Notice)) {
                    info!("chat {:?}: {}", event.chat_type, event.message);

                    let life = if matches!(event.chat_type, ChatType::Announce | ChatType::AdminChat) { LONG_CHAT } else { SHORT_CHAT };

                    self.chat.push(ChatLine {
                        kind: event.chat_type,
                        speaker: event.speaker_id,
                        text: event.message.chars().take(CHAT_LENGTH).collect(),
                        volume: event.volume,
                        life,
                    });

                    if self.chat.len() > CHAT_LINES {
                        self.chat.remove(0);
                    }
                }
            }
            ServerEvent::UpdateVehicleTypeColor(event) => {
                if let Some(vehicle) = self.vehicles.get_mut(event.vehicle_id as usize) {
                    vehicle.active = true;
                    vehicle.health = 100;
                    vehicle.kind = event.vehicle_type;

                    vehicle.color = if event.vehicle_color == 0xf {
                        u32::MAX
                    } else {
                        event.vehicle_color as u32
                    };

                    vehicle.track.tick = -1;
                    vehicle.windows_broken = [false; 8];
                    vehicle.burst = 0;
                }
            }
            ServerEvent::UpdateVehicle(event) => {
                let Some(vehicle) = self.vehicles.get_mut(event.vehicle_id as usize) else {
                    return;
                };

                match event.kind {
                    0 | 6 => {
                        if let Some(w) = usize::try_from(event.part)
                            .ok()
                            .and_then(|w| vehicle.windows_broken.get_mut(w))
                        {
                            *w = event.kind == 0;
                        }
                    }
                    1 => vehicle.burst |= u32::try_from(event.part).map_or(0, |w| 1u32.wrapping_shl(w)),
                    2 => vehicle.health = event.part,
                    3 => {
                        if let Some(t) = usize::try_from(vehicle.traffic_car)
                            .ok()
                            .and_then(|c| self.traffic.get_mut(c))
                        {
                            t.vehicle = -1;
                        }

                        vehicle.active = false;
                    }
                    _ => {}
                }
            }
            ServerEvent::UpdateItem(event) => {
                let id = event.item_id as usize & (ITEMS - 1);
                let item = &mut self.items[id];

                if event.active && !item.active {
                    item.track = Track::default();
                }

                item.active = event.active;
                item.kind = ItemKind::try_from(event.item_type).ok();
                item.parent_human = event.parent_human;
                item.parent_item = event.parent_item;
                item.parent_slot = event.parent_slot as u32;

                self.decoder.set_item_kind(id, event.item_type as u32);
            }
            ServerEvent::UpdatePhone(event) => {
                self.items[event.item_id as usize & (ITEMS - 1)].phone =
                    (event.phone_status, event.display_phone_number, event.phone_texture);
            }
            ServerEvent::UpdatePlayer(event) => {
                let player = &mut self.players[event.client_id as usize & (PLAYERS - 1)];

                player.active = event.active;

                if player.active {
                    player.is_bot = event.is_bot;
                    player.team = event.team;
                    player.human = event.human_id;
                    player.look = event.customization;
                    player.name = event.name;
                }
            }
            ServerEvent::UpdateCorporation(event) => {
                let slot = (event.a >> 1 & 7) as usize;
                let Some(m) = self.missions.get_mut((event.a >> 4 & 0xf) as usize).and_then(|c| c.get_mut(slot)) else { return };

                m.active = event.a & 1 != 0;

                if m.active {
                    m.kind = event.a >> 8 & 0xf;
                    m.disk_type = event.a >> 16 & 0xff;
                    m.team1 = event.b & 0xf;
                    m.team2 = event.b >> 4 & 0xf;
                    m.value = event.b >> 8;
                    m.deadline = (event.c & 0xffff) * 60;
                    m.location = (event.c as u32 >> 16) as i32 - 1;
                    m.cash = event.d;
                }
            }
            ServerEvent::MissionText(event) => {
                if let Some(m) = self.missions.get_mut((event.corporation & 0xf) as usize).and_then(|c| c.get_mut((event.mission & 0xf) as usize)) {
                    m.active = true;
                }
            }
            ServerEvent::Mission(event) if event.player == self.player => {
                let byte = |shift: i32| event.value >> shift & 0xff;

                match event.kind {
                    ROLE => {
                        self.role = event.role;

                        if event.role == PROTECTOR {
                            self.saviors = (byte(0), byte(10));
                        }
                    }
                    TARGET => self.target = Some(Vec3::from(event.pos)),
                    RESULT => {
                        let k = (event.role & 7) as usize;
                        self.results[k] = [byte(0), byte(10), byte(20)];
                        self.result_count = self.result_count.max(k + 1);
                    }
                    _ => {}
                }
            }
            ServerEvent::UpdateStock(event) => {
                let [a, b, c] = event.prices;
                self.share_prices[..5].copy_from_slice(&[a & 0xffff, a >> 16, b & 0xffff, b >> 16, c & 0xffff]);
            }
            ServerEvent::UpdatePlayerRound(event) => {
                let player = &mut self.players[event.client_id as usize & (PLAYERS - 1)];

                player.money = event.money;
                player.stocks = event.stocks;
                player.phone_number = event.phone_number as i32;
                player.muted = self.mute_list.contains(&player.phone_number);
            }
            ServerEvent::ItemSetCell(event) if event.area == 0 => {
                self.cell_changes.push(CellChange::Taken { cell: IVec3::new(event.block_x, event.block_y, event.block_z), mask: event.taken });
            }
            ServerEvent::SetBlockCell(event) if event.area == 0 => {
                self.cell_changes.push(CellChange::ItemSet { cell: IVec3::new(event.block_x, event.block_y, event.block_z), value: event.value });
            }
            ServerEvent::TeamDoor(event) => {
                if let Ok(team) = usize::try_from(event.team_id) {
                    if self.team_doors.len() <= team {
                        self.team_doors.resize(team + 1, false);
                    }

                    self.team_doors[team] = event.door_open;
                }
            }
            ServerEvent::UpdateHuman(event) => {
                let Some(human) = self.humans.get_mut(event.human_id as usize) else {
                    return;
                };

                human.active = event.active;

                if let Some(p) = usize::try_from(event.player_id)
                    .ok()
                    .and_then(|p| self.players.get_mut(p))
                {
                    p.human = if event.active { event.human_id } else { -1 };
                }

                if event.active {
                    human.player = event.player_id;
                }
            }
            _ => {}
        }
    }
}