pub const IKEA_SWITCH_TOPIC: &str = "zigbee2mqtt/ikea-bryter";
pub const PHILLIPS_SWITCH_TOPIC: &str = "zigbee2mqtt/phillips-bryter";

pub const GET_MAIN_LIGHT_TOPIC: &str = "zigbee2mqtt/taklys";
pub const GET_TOP_LIGHT_TOPIC: &str = "zigbee2mqtt/topplys";
pub const GET_MID_LIGHT_TOPIC: &str = "zigbee2mqtt/midtlys";
pub const GET_NIGHT_LIGHT_TOPIC: &str = "zigbee2mqtt/nattlys";

pub const SET_MAIN_LIGHT_TOPIC: &str = "zigbee2mqtt/taklys/set";
pub const SET_TOP_LIGHT_TOPIC: &str = "zigbee2mqtt/topplys/set";
pub const SET_MID_LIGHT_TOPIC: &str = "zigbee2mqtt/midtlys/set";
pub const SET_NIGHT_LIGHT_TOPIC: &str = "zigbee2mqtt/nattlys/set";

pub const TOPICS: [&str; 6] = [
    "zigbee2mqtt/ikea-bryter",
    "zigbee2mqtt/phillips-bryter",
    "zigbee2mqtt/taklys",
    "zigbee2mqtt/topplys",
    "zigbee2mqtt/midtlys",
    "zigbee2mqtt/nattlys",
];
pub const SET_TOPICS: [&str; 4] = [
    "zigbee2mqtt/taklys/set",
    "zigbee2mqtt/topplys/set",
    "zigbee2mqtt/midtlys/set",
    "zigbee2mqtt/nattlys/set",
];

#[derive(Debug, Copy, Clone)]
pub enum TargetLight {
    MainLight,
    TopLight,
    MidLight,
    NightLight,
}

#[derive(Debug, Copy, Clone)]
pub enum Select<T> {
    Current,
    All,
    Specific(T),
    None,
}
impl<T> Default for Select<T> {
    fn default() -> Select<T> {
        Select::All
    }
}

#[derive(Debug, Copy, Clone)]
pub struct LightState {
    pub is_on: bool,
    pub brightness: u8,
    pub color: Color,
}

impl Default for LightState {
    fn default() -> LightState {
        LightState {
            is_on: true,
            color: Color::default(),
            brightness: 254,
        }
    }
}

impl LightState {
    pub fn on() -> LightState {
        LightState {
            is_on: true,
            color: Color::default(),
            brightness: 254,
        }
    }
    pub fn off() -> LightState {
        LightState {
            is_on: false,
            color: Color::default(),
            brightness: 0,
        }
    }
}

/// Color given in hue, saturation and CIE 1931 Color space coordinate (xy).
#[derive(Debug, Copy, Clone)]
pub struct Color {
    pub x: f32,
    pub y: f32,
}
impl Default for Color {
    fn default() -> Color {
        Color { x: 0.34, y: 0.34 }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct LightCommand {
    pub target_light: Select<TargetLight>,
    pub target_state: LightState,
}

impl LightCommand {
    pub fn topics(&self) -> Option<Box<[String]>> {
        match self.target_light {
            Select::All => Some(
                SET_TOPICS
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
                    .into_boxed_slice(),
            ),
            Select::Specific(light) => Some(
                vec![
                    match light {
                        TargetLight::TopLight => SET_TOP_LIGHT_TOPIC,
                        TargetLight::MidLight => SET_MID_LIGHT_TOPIC,
                        TargetLight::MainLight => SET_MAIN_LIGHT_TOPIC,
                        TargetLight::NightLight => SET_NIGHT_LIGHT_TOPIC,
                    }
                    .to_string(),
                ]
                .into_boxed_slice(),
            ),
            _ => None,
        }
    }

    pub fn as_json(&self) -> String {
        let mut final_json = "{".to_string();

        match self.target_state.is_on {
            true => final_json.push_str("\"state\" : \"ON\", "),
            false => final_json.push_str("\"state\" : \"OFF\", "),
        }

        final_json
            .push_str(format!("\"brightness\" : {}, ", self.target_state.brightness).as_str());

        final_json.push_str("\"color_mode\":\"color_temp\",\"color_temp\":");
        final_json.push_str(format!("{}", 250).as_str());

        final_json.push('}');

        final_json
    }
}

impl Default for LightCommand {
    fn default() -> LightCommand {
        LightCommand {
            target_light: Select::None,
            target_state: LightState::default(),
        }
    }
}
