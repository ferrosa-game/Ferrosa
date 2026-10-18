use ferrosa_protocol::Team;

pub const TEAM_COLOURS: [[f32; 3]; 8] =
    [[0.75, 0.5625, 0.1875], [0.1875, 0.375, 0.5625], [0.25, 0.25, 0.25], [0.5625, 0.1875, 0.1875], [0.875, 0.875, 0.875], [0.125, 0.5, 0.125], [0.75, 0.75, 0.75], [0.25, 0.25, 0.25]];

pub fn phone_number(number: u32) -> String {
    format!("{:03}-{:04}", number / 10000, number % 10000)
}

pub fn corporation_index(team: Team) -> Option<usize> {
    Team::CORPORATIONS.iter().position(|&t| t == team)
}

pub fn corporation_name(team: Team) -> Option<&'static str> {
    corporation_index(team).map(|k| Team::CORPORATION_NAMES[k])
}
