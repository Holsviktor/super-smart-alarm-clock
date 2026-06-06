use serde::{Deserialize, Deserializer};
use std::fmt;

#[derive(Deserialize, Debug)]
pub struct ButtonMessage {
    pub action: ButtonAction,
}

#[derive(Deserialize, Debug)]
pub enum ControllerButton {
    On,
    Off,
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug)]
pub enum ButtonAction {
    Press(ControllerButton),
    Release(ControllerButton),
}
impl<'de> Deserialize<'de> for ButtonAction {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ButtonActionVisitor;

        impl<'de> serde::de::Visitor<'de> for ButtonActionVisitor {
            type Value = ButtonAction;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "action")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ButtonAction, E> {
                match v.to_lowercase().as_str() {
                    // Ikea Controller
                    "on" => Ok(ButtonAction::Press(ControllerButton::On)),
                    "off" => Ok(ButtonAction::Press(ControllerButton::Off)),
                    "arrow_left_click" => Ok(ButtonAction::Press(ControllerButton::Left)),
                    "arrow_right_click" => Ok(ButtonAction::Press(ControllerButton::Right)),

                    // Phillips Controller
                    "on_press" => Ok(ButtonAction::Press(ControllerButton::On)),
                    "off_press" => Ok(ButtonAction::Press(ControllerButton::Off)),
                    "up_press" => Ok(ButtonAction::Press(ControllerButton::Up)),
                    "down_press" => Ok(ButtonAction::Press(ControllerButton::Down)),

                    "on_press_release" => Ok(ButtonAction::Release(ControllerButton::On)),
                    "off_press_release" => Ok(ButtonAction::Release(ControllerButton::Off)),
                    "up_press_release" => Ok(ButtonAction::Release(ControllerButton::Up)),
                    "down_press_release" => Ok(ButtonAction::Release(ControllerButton::Down)),

                    _ => Err(E::unknown_variant(v, &["action"])),
                }
            }
        }

        d.deserialize_str(ButtonActionVisitor)
    }
}
