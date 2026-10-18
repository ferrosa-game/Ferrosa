use bevy::prelude::*;
use ferrosa_protocol::{LeaveGame, frame_packet, serverbound::game::{ClientGamePacket, actions::GameAction, voice::{VoiceData, VoiceFrame}}};

use crate::{game::netgame::{ACTIONS, ACTIONS_SENT, MUTE_LIST, NetGame, VOICE_FRAMES, VOICE_SENT}, net::ClientSocket};

impl NetGame {
    pub fn send(&mut self, socket: &mut ClientSocket, camera: Vec3, now: u32) {
        let Some(pl) = self.me() else { return };

        let (head, tail) = (pl.action_head, pl.last_actions);

        let count = if head < tail {
            head + ACTIONS as i32 - tail
        } else {
            head - tail
        }
        .min(ACTIONS_SENT);

        let actions = (0..count)
            .map(|k| {
                pl.actions[((tail + k) as usize) & (ACTIONS - 1)]
                    .clone()
                    .unwrap_or(GameAction::Unknown)
            })
            .collect();

        let frames = std::array::from_fn(|k| {
            let f = (self.voice_out_next + VOICE_FRAMES - VOICE_SENT + k) % VOICE_FRAMES;
            VoiceFrame { index: f as u8, ..self.voice_out[f].clone() }
        });

        let p = ClientGamePacket {
            round_num: self.round,
            gear_x: pl.gear_x,
            left_right: pl.left_right,
            gear_y: pl.gear_y,
            forward_back: pl.forward_back,
            view_yaw_delta: pl.view_yaw_delta,
            view_pitch: pl.view_pitch,
            free_look_yaw: pl.free_look_yaw,
            free_look_pitch: pl.free_look_pitch,
            view_yaw: pl.view_yaw,
            spin_yaw: pl.spin_yaw,
            view_pitch_delta: pl.view_pitch_delta,
            input_flags: pl.input_flags,
            input_type: pl.input_type as u8,
            zoom_level: pl.zoom_level as u8,
            received_events: self.events_played,
            num_sent_objects: self.decoder.object_packet,
            camera_pos: camera,
            packet_action_count: count as u8,
            total_actions: tail as u8,
            actions,
            voice_data: VoiceData {
                frames,
                is_silenced: self.silenced,
            },
            spectating_human_id: self.spectating as u8,
            pack_ack: self.decoder.packs_applied,
            link_ack: self.decoder.links_received,
            packet_count: self.sent,
            sdl_tick: now,
        };

        self.sent = self.sent.wrapping_add(1);

        socket.send(&frame_packet(p), self.server);
    }

    pub fn leave(&self, socket: &mut ClientSocket) {
        socket.send(&frame_packet(LeaveGame), self.server);
    }

    pub fn toggle_mute(&mut self, k: usize) {
        let Some(p) = self.players.get_mut(k) else { return };
        let number = p.phone_number;

        p.muted = !p.muted;

        if p.muted {
            if !self.mute_list.contains(&number) && self.mute_list.len() < MUTE_LIST {
                self.mute_list.push(number);
            }
        } else {
            self.mute_list.retain(|&n| n != number);
        }
    }
}