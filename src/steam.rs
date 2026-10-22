#[derive(bevy::prelude::Resource)]
pub struct Steam {
    pub client: Option<steamworks::Client>,
    pub ticket: Option<Vec<u8>>,
    pub requested_ticket: bool,
}

impl Steam {
    pub fn init() -> Self {
        Self { client: steamworks::Client::init_app(272230).ok(), ticket: None, requested_ticket: false }
    }

    pub fn ok(&self) -> bool {
        self.client.is_some()
    }

    pub fn run_callbacks(&mut self) {
        let Some(client) = &self.client else { return };

        client.run_callbacks();

        if self.requested_ticket && self.ticket.is_none() {
            let mut buf = [0u8; 1024];
            let mut len = 0u32;

            let got = unsafe {
                steamworks::sys::SteamAPI_ISteamUser_GetEncryptedAppTicket(
                    steamworks::sys::SteamAPI_SteamUser_v023(),
                    buf.as_mut_ptr().cast(),
                    1024,
                    &mut len
                )
            };

            if got && len as usize <= 1024 {
                self.ticket = Some(buf[..len as usize].to_vec());
            }
        }
    }

    pub fn persona_name(&self) -> Option<String> {
        self.client.as_ref().map(|c| c.friends().name())
    }

    pub fn user_id(&mut self) -> Option<u64> {
        let c = self.client.as_ref()?;
        let (owner, user) = (c.apps().app_owner().raw(), c.user().steam_id().raw());

        if user == owner {
            return Some(user);
        }

        self.client = None;

        None
    }

    pub fn request_encrypted_app_ticket(&mut self) {
        if self.client.is_none() {
            return;
        }

        self.ticket = None;

        unsafe {
            steamworks::sys::SteamAPI_ISteamUser_RequestEncryptedAppTicket(steamworks::sys::SteamAPI_SteamUser_v023(), std::ptr::null_mut(), 0);
        }

        self.requested_ticket = true;
    }
}
