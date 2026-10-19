use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

const FRAGMENT_OVER: usize = 0x4b0;
const FRAGMENT: usize = 0x400;
const FRAGMENT_SLOTS: usize = 64;
const MAX_FRAGMENTS: usize = 16;

#[derive(Clone, Debug)]
pub struct Packet {
    pub data: Vec<u8>,
    pub from: SocketAddr,
}

#[derive(Clone, Debug, Default)]
struct Reassembly {
    id: u16,
    received: u32,
    len: usize,
    data: Vec<u8>,
}

#[derive(bevy::prelude::Resource)]
pub struct ClientSocket {
    socket: Option<UdpSocket>,
    fragment_id: u16,
    slots: Vec<Reassembly>,
    first: usize,
    next: usize,
}

impl ClientSocket {
    pub fn open() -> Self {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok().filter(|s| s.set_nonblocking(true).is_ok());

        if let Some(s) = &socket {
            let _ = s.set_broadcast(true);
        }

        Self { socket, fragment_id: 0, slots: vec![Reassembly::default(); FRAGMENT_SLOTS], first: 0, next: 0 }
    }

    pub fn send(&mut self, data: &[u8], to: SocketAddrV4) {
        let Some(socket) = &self.socket else { return };

        if data.len() <= FRAGMENT_OVER {
            let _ = socket.send_to(data, to);
            return;
        }

        let count = (data.len() >> 10) + 1;

        for i in 0..count {
            let chunk = &data[(i * FRAGMENT).min(data.len())..((i + 1) * FRAGMENT).min(data.len())];
            let mut f = b"7DFF".to_vec();

            f.extend_from_slice(&self.fragment_id.to_le_bytes());
            f.push(i as u8);
            f.push(count as u8);
            f.extend_from_slice(chunk);

            let _ = socket.send_to(&f, to);
        }

        self.fragment_id = self.fragment_id.wrapping_add(1);
    }

    pub fn recv(&mut self) -> Option<Packet> {
        let mut buf = vec![0u8; 0x10000];

        loop {
            let (n, from) = self.socket.as_ref()?.recv_from(&mut buf).ok()?;

            if n < 4 || &buf[..3] != b"7DF" {
                continue;
            }

            match buf[3] {
                b'P' => return Some(Packet { data: buf[..n].to_vec(), from }),
                b'F' => {
                    if let Some(data) = self.reassemble(&buf[4..n]) {
                        return Some(Packet { data, from });
                    }
                }
                _ => {}
            }
        }
    }

    fn reassemble(&mut self, f: &[u8]) -> Option<Vec<u8>> {
        if f.len() < 4 {
            return None;
        }

        let id = u16::from_le_bytes([f[0], f[1]]);
        let (index, count) = (f[2] as usize, f[3] as usize);

        if count > MAX_FRAGMENTS || index > MAX_FRAGMENTS - 1 {
            return None;
        }

        let mut k = self.first;

        let slot = loop {
            if k == self.next {
                let s = self.next;
                self.slots[s] = Reassembly { id, received: 0, len: 0, data: vec![0; MAX_FRAGMENTS * FRAGMENT] };
                self.next = (s + 1) % FRAGMENT_SLOTS;

                if self.next == self.first {
                    self.first = (self.first + 1) % FRAGMENT_SLOTS;
                }

                break s;
            }

            if self.slots[k].id == id {
                break k;
            }

            k = (k + 1) % FRAGMENT_SLOTS;
        };

        let s = &mut self.slots[slot];
        let body = &f[4..];
        let at = index * FRAGMENT;

        s.data[at..at + body.len()].copy_from_slice(body);
        s.len = s.len.max(at + body.len());
        s.received |= 1 << index;

        let all = (1u32 << count) - 1;

        (s.received & all == all).then(|| s.data[..s.len].to_vec())
    }
}
