use crate::device_manager::ControlMessage;
use crate::integrations::pipeweaver::channel::{ChannelConfig, PipeweaverChannel};
use crate::integrations::pipeweaver::layout::{
    BACKGROUND_COLOUR, DISPLAY_DIMENSIONS, DrawingUtils, JPEG_QUALITY,
};
use beacn_lib::crossbeam;
use beacn_lib::crossbeam::channel::Sender;
use enum_map::enum_map;
use image::{Rgba, RgbaImage, imageops};
use log::debug;
use pipeweaver_shared::{Mix, MuteTarget};

mod channel;
mod layout;

pub fn perform_test_render(sender: Sender<ControlMessage>) {
    let test_config_1: ChannelConfig = ChannelConfig {
        title: "Microphone".to_string(),
        colour: Rgba([47, 24, 71, 255]),
        volumes: enum_map! {
            Mix::A => 100,
            Mix::B => 100,
        },
        active_mix: Mix::A,
        mute_states: enum_map! {
            MuteTarget::TargetA => channel::MuteState {
                is_active: false,
                is_mute_to_all: true,
            },
            MuteTarget::TargetB => channel::MuteState {
                is_active: false,
                is_mute_to_all: true
            }
        },
    };
    let test_config_2: ChannelConfig = ChannelConfig {
        title: "System".to_string(),
        colour: Rgba([153, 98, 30, 255]),
        volumes: enum_map! {
            Mix::A => 50,
            Mix::B => 20,
        },
        active_mix: Default::default(),
        mute_states: enum_map! {
            MuteTarget::TargetA => channel::MuteState {
                is_active: false,
                is_mute_to_all: false,
            },
            MuteTarget::TargetB => channel::MuteState {
                is_active: true,
                is_mute_to_all: true
            }
        },
    };
    let test_config_3: ChannelConfig = ChannelConfig {
        title: "Browser".to_string(),
        colour: Rgba([211, 139, 93, 255]),
        volumes: enum_map! {
            Mix::A => 82,
            Mix::B => 76,
        },
        active_mix: Default::default(),
        mute_states: enum_map! {
            MuteTarget::TargetA => channel::MuteState {
                is_active: false,
                is_mute_to_all: true,
            },
            MuteTarget::TargetB => channel::MuteState {
                is_active: false,
                is_mute_to_all: true
            }
        },
    };
    let test_config_4: ChannelConfig = ChannelConfig {
        title: "Game".to_string(),
        colour: Rgba([243, 255, 182, 255]),
        volumes: enum_map! {
            Mix::A => 80,
            Mix::B => 60,
        },
        active_mix: Default::default(),
        mute_states: enum_map! {
            MuteTarget::TargetA => channel::MuteState {
                is_active: false,
                is_mute_to_all: false,
            },
            MuteTarget::TargetB => channel::MuteState {
                is_active: true,
                is_mute_to_all: true
            }
        },
    };

    // Register the handlers, and prepare an initial state
    let dimensions = DISPLAY_DIMENSIONS;
    let background = BACKGROUND_COLOUR;
    let mut base_img = RgbaImage::from_pixel(dimensions.0, dimensions.1, background);

    // Ok, this does nothing except render demo data on the screen
    let channels: Vec<PipeweaverChannel> = vec![
        PipeweaverChannel::new(0, sender.clone(), test_config_1),
        PipeweaverChannel::new(1, sender.clone(), test_config_2),
        PipeweaverChannel::new(2, sender.clone(), test_config_3),
        PipeweaverChannel::new(3, sender.clone(), test_config_4),
    ];

    for channel in &channels {
        let initial = channel.get_initial();
        imageops::overlay(
            &mut base_img,
            &initial.image,
            initial.x as i64,
            initial.y as i64,
        );
    }

    // Send this image to the display
    if let Ok(jpeg_data) = DrawingUtils::image_as_jpeg(base_img, background, JPEG_QUALITY) {
        let (tx, rx) = oneshot::channel();
        let _ = sender.send(ControlMessage::SendImage(jpeg_data, 0, 0, tx));
        let _ = rx.recv();
    }
}
