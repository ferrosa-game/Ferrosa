use bevy::prelude::*;
use ferrosa_protocol::{clientbound::{game::{GameState, ItemKind, ItemTail, MenuType, ServerGamePacket, decode::SlotChange}, initial_sync::InitialSync}, codec::{Reader, WireRead}};

use crate::{game::netgame::{ADMIN_NAME, EVENTS, GAP, HUMANS, INVENTORY_SLOTS, ITEMS, LINKS, MAX_ITEM_CHILDREN, MEMO_LINE, MEMO_SIZE, NetGame, PLAYERS, SCREEN_COLUMNS, SCREEN_LINES, STALE, TIMEOUT, TRAFFIC, VEHICLES, VOICE_FRAMES, VOICE_RECEIVED, entities::Item, sync::ServerSync}, net::ClientSocket};

impl NetGame {
    pub fn receive(&mut self, socket: &mut ClientSocket, now: u32) -> bool {
        self.silence += 1;

        if self.silence > TIMEOUT {
            return false;
        }

        while let Some(packet) = socket.recv() {
            if packet.from == std::net::SocketAddr::V4(self.server) {
                self.packet(&packet.data[4..], now);
            }
        }

        true
    }

    fn packet(&mut self, data: &[u8], now: u32) {
        let mut r = Reader::new(data);

        match r.u8() {
            Ok(6) => {
                self.silence = 0;

                if let Ok(sync) = InitialSync::read(&mut r) {
                    self.sync = ServerSync::from(sync);
                    self.server_round = self.sync.round;
                }
            }
            Ok(5) => {
                self.silence = 0;

                let (round, tick) = (self.round, self.tick);

                let accept =
                    |r: u32, t: u32| r == round && !(tick > t as i32 && tick - t as i32 <= STALE);

                match ServerGamePacket::read(&mut r, &mut self.decoder, accept) {
                    Ok(Some(p)) => {
                        self.note_gap(now);
                        self.dropped = 0;
                        self.apply(p, now);
                    }
                    Ok(None) => self.note_drop("it is behind us or from another round"),
                    Err(e) => self.note_drop(&format!("it would not decode: {e:?}")),
                }
            }
            Ok(9) => {
                if let Some(p) = self.me_mut() {
                    p.is_admin = true;
                }

                self.read_admin_list(&mut r);
            }
            _ => {}
        }
    }

    fn note_drop(&mut self, why: &str) {
        self.dropped += 1;

        if self.dropped.is_power_of_two() {
            warn!("dropped {} game packets in a row, the last because {why}", self.dropped);
        }
    }

    fn note_gap(&mut self, now: u32) {
        let since = now.wrapping_sub(self.last_applied);

        if self.last_applied > 0 && since > GAP {
            warn!("{since}ms without a game packet we could use, {} of them dropped", self.dropped);
        }

        self.last_applied = now;
    }

    fn apply(&mut self, p: ServerGamePacket, now: u32) {
        self.tick = p.ticks_since_reset as i32;
        self.game_state = p.gamestate;

        if let Some(ready) = p.ready_states {
            self.players
                .iter_mut()
                .zip(ready)
                .for_each(|(pl, r)| pl.is_ready = r);
        }

        if matches!(p.gamestate, GameState::Intermission | GameState::Restarting) {
            self.corp_round = p.corp_round;
        }

        self.timer = p.game_timer as u32;
        self.timer_extra = p.game_timer_extra as u32;
        self.sun_time = p.sun_time as u32;
        self.team_counts = p.team_counts.map(|n| n as u32);
        self.player = p.client_id as i32;
        let human = p.own_human.as_ref().map_or(-1, |h| h.human_id);
        self.own_human = p.own_human.clone();

        let my_human = &mut self.players[p.client_id as usize & (PLAYERS - 1)];

        my_human.human = human;
        my_human.manager_tab = p.manager_tab;
        my_human.menu = p.menu_type;
        my_human.menu_tab = p.menu_tab;
        my_human.money = p.money;
        my_human.team_money = p.corp_money;
        my_human.budget = p.corp_credit;
        my_human.corporate_rating = p.corp_rating;
        my_human.criminal_rating = p.crim_rating;
        my_human.spawn_timer = p.spawn_timer;
        my_human.last_actions = p.received_actions as i32;
        my_human.vitality = p.vitality;
        my_human.display_inventory = p.display_inventory.clone();
        my_human.menu_buttons = p.menu_buttons.clone();

        if p.menu_type == MenuType::WorldStore {
            self.shops.insert(p.menu_tab, p.shop.clone());
        }

        self.objects(&p);
        self.line_links(&p);

        for (channel, sent) in self.voice.iter_mut().zip(&p.voice) {
            let Some(sent) = sent else {
                channel.active = false;
                channel.frames.iter_mut().for_each(|f| f.size = 0);
                continue;
            };

            (channel.active, channel.player, channel.human, channel.item) = (true, sent.player_id, sent.human_id, sent.item_id);

            for f in &sent.voice_frames {
                channel.frames[f.index as usize % VOICE_FRAMES] = f.clone();
            }

            channel.next = (sent.voice_frames[VOICE_RECEIVED - 1].index as usize + 1) % VOICE_FRAMES;
        }
        self.server_events = p.global_event_count;

        for (id, e) in p.events {
            self.events[id as usize & (EVENTS - 1)] = Some(e);
            if self.events_received == id {
                self.events_received = (self.events_received + 1) & 0xffff;
            }
        }

        self.server_packet = p.packet_sequence;
        self.ping = now.wrapping_sub(p.last_sdl_tick);

        self.rebuild_inventories();
    }

    fn objects(&mut self, p: &ServerGamePacket) {
        for c in std::mem::take(&mut self.decoder.changes) {
            match c {
                SlotChange::Pack { kind: 0, index, .. } => {
                    if let Some(h) = self.humans.get_mut(index as usize) {
                        h.active = true;
                    }
                }
                SlotChange::Pack {
                    kind: 1,
                    item_type,
                    index,
                } => {
                    if let Some(it) = self.items.get_mut(index as usize) {
                        *it = Item {
                            active: true,
                            kind: u8::try_from(item_type).ok().and_then(|t| ItemKind::try_from(t).ok()),
                            ..default()
                        };
                    }
                }
                SlotChange::Unpack { kind: 0, index } => {
                    if let Some(h) = self.humans.get_mut(index as usize) {
                        h.active = false;
                    }
                }
                SlotChange::Unpack { kind: 1, index } => {
                    if let Some(it) = self.items.get_mut(index as usize) {
                        it.active = false;
                    }
                }
                _ => {}
            }
        }

        let tick = self.tick;
        let quat = |q: [f32; 4]| Quat::from_xyzw(q[0], q[1], q[2], q[3]);

        for obj in &p.humans {
            let human = &mut self.humans[obj.human_id as usize & (HUMANS - 1)];

            if let Some(a) = &obj.appearance {
                human.player = a.player_id;
                human.look = a.customization;
                human.wanted = a.wanted as u32;
            }

            human.vehicle = obj.vehicle;
            human.vehicle_seat = obj.vehicle_seat as u32;
            human.alive = obj.alive;
            human.bleeding = obj.bleeding;
            human.track.moved(tick, obj.pos);
            human.rot = quat(obj.rot);
            human.bones = obj.bones.map(quat);
        }

        for object in &p.items {
            let item = &mut self.items[object.item_id as usize & (ITEMS - 1)];

            let shown = (item.current_line, item.top_line, item.cursor);

            if let Some(hd) = &object.header {
                item.kind = object.item_type;
                item.parent_item = hd.parent_item;
                item.parent_human = hd.parent_human;
                item.parent_slot = hd.parent_slot as u32;

                if let Some((current, top)) = hd.screen {
                    (item.current_line, item.top_line) = (current, top);
                }
            }

            match object.tail {
                ItemTail::Radio(on) => item.top_line = on as i32,
                ItemTail::Computer(cursor) => item.cursor = cursor,
                _ => {}
            }

            if shown != (item.current_line, item.top_line, item.cursor) {
                item.text_version = item.text_version.wrapping_add(1);
            }

            if item.has_history {
                item.track.moved(tick, object.pos);
            } else {
                item.track.reset(tick, object.pos);
                item.has_history = true;
            }

            item.rot = quat(object.rot);
            item.tail = object.tail;
        }

        for obj in &p.vehicles {
            let id = obj.vehicle_id as usize & (VEHICLES - 1);
            let car = usize::try_from(obj.traffic_car).ok().filter(|&c| c < TRAFFIC);
            let vehicle = &mut self.vehicles[id];

            vehicle.traffic_car = obj.traffic_car;
            vehicle.track.moved(tick, obj.pos);
            vehicle.rot = quat(obj.rot);

            if let Some(car) = car.filter(|&c| vehicle.active && self.traffic[c].vehicle != id as i32) {
                let t = &mut self.traffic[car];
                t.vehicle = id as i32;
                vehicle.kind = t.kind;
                vehicle.color = t.color;
                vehicle.track.reset(tick, obj.pos);
            }

            vehicle.steer = (obj.steer as f32 as f64 * std::f64::consts::PI / 255.0) as f32;
            vehicle.wheels = obj.wheels.map(|w| w.map(|v| v as f32 / 255.0));
            vehicle.engine_rpm = obj.engine_rpm as u32;
        }

        self.traffic_count = p.traffic_count as u32;

        for obj in &p.traffic {
            let traffic_car = &mut self.traffic[obj.index as usize & (TRAFFIC - 1)];
            traffic_car.track.moved(tick, obj.pos);

            if !obj.vehicle {
                traffic_car.vehicle = -1;
            }

            if let Some((kind, color)) = obj.kind_color {
                traffic_car.kind = kind;
                traffic_car.color = color as u32;
            }

            traffic_car.yaw = obj.yaw;

            let (vehicle, kind, color) = (traffic_car.vehicle, traffic_car.kind, traffic_car.color);
            if let Some(ve) = usize::try_from(vehicle)
                .ok()
                .and_then(|v| self.vehicles.get_mut(v))
            {
                ve.kind = kind;
                ve.color = color;
            }
        }

        if let Some(s) = self.signals.get_mut(p.signal.0 as usize) {
            *s = p.signal.1;
        }
    }

    fn read_admin_list(&mut self, r: &mut Reader) {
        for row in &mut self.admin_list {
            let Ok(filled) = r.bits(1) else { return };
            row.filled = filled != 0;

            if row.filled {
                let (Ok(account), Ok(name), Ok(phone)) = (r.i32(), r.bytes(ADMIN_NAME), r.i32()) else { return };
                row.account = account;
                row.name = String::from_utf8_lossy(name).trim_end_matches('\0').to_string();
                row.phone_number = phone;
            }
        }
    }

    fn line_links(&mut self, p: &ServerGamePacket) {
        for l in &p.line_links {
            let index = (l.index as usize).min(LINKS - 1);

            if self.link_ticks[index].is_some_and(|t| t > l.tick) {
                continue;
            }

            self.link_ticks[index] = Some(l.tick);

            let Some(item) = usize::try_from(l.item).ok().and_then(|i| self.items.get_mut(i)) else { continue };
            let line = l.line.max(0) as usize;

            match l.kind {
                0 => {
                    let at = line * MEMO_LINE;
                    let n = l.text.len().min(MEMO_SIZE.saturating_sub(at));
                    item.memo[at..at + n].copy_from_slice(&l.text[..n]);
                    item.memo[MEMO_SIZE - 1] = 0;
                }
                1 if line < SCREEN_LINES => {
                    let n = l.text.len().min(SCREEN_COLUMNS);

                    if item.screen.len() <= line {
                        item.screen.resize(line + 1, (Vec::new(), Vec::new()));
                    }

                    item.screen[line] = (l.text[..n].to_vec(), l.colors.iter().copied().take(n).collect());
                }
                _ => continue,
            }

            item.text_version = item.text_version.wrapping_add(1);
        }
    }

    fn rebuild_inventories(&mut self) {
        self.humans
            .iter_mut()
            .for_each(|h| h.inventory.iter_mut().for_each(Vec::clear));

        self.items.iter_mut().for_each(|i| i.children.clear());

        for i in 0..ITEMS {
            let it = &self.items[i];

            if !it.active {
                continue;
            }

            let (parent, human, slot) = (it.parent_item, it.parent_human, it.parent_slot as usize);

            if let Some(p) = usize::try_from(parent)
                .ok()
                .map(|p| &mut self.items[p & (ITEMS - 1)])
                .filter(|p| p.children.len() < MAX_ITEM_CHILDREN)
            {
                p.children.push(i as i32);
            }

            if let Some(inv) = usize::try_from(human)
                .ok()
                .filter(|_| slot < INVENTORY_SLOTS)
                .map(|h| &mut self.humans[h & (HUMANS - 1)].inventory[slot])
                && inv.len() <= 1
            {
                inv.push(i as i32);
            }
        }
    }
}