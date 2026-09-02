use crate::devices::manager::DefinitionState;
use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::devices::states::control::ControlState;
use crate::ui::app::DeviceState;
use crate::ui::pages::audio::about::AboutMessage;
use crate::ui::pages::audio::config::ConfigMessage;
use crate::ui::pages::audio::hp_equaliser::HPEQMessage;
use crate::ui::pages::audio::lighting::LightingMessage;
use crate::ui::pages::audio::studio_link::StudioLinkMessage;
use crate::ui::pages::common::error_page::ErrorPageMessages;
use crate::ui::pages::control::about::ControlAboutMessage;
use iced::{Element, Task};

#[derive(Debug, Clone)]
pub(crate) enum PageMessage {
    AudioAbout(AboutMessage),
    AudioConfig(ConfigMessage),
    AudioLighting(LightingMessage),
    AudioStudioLink(StudioLinkMessage),
    AudioHPEqualiser(HPEQMessage),

    ControlAbout(ControlAboutMessage),

    ErrorPage(ErrorPageMessages),
}

pub(crate) trait Page {
    /// The icon to show in the left panel
    fn icon(&self) -> &'static str {
        "error"
    }

    /// Whether this page should be shown at all, defaults to Device = Working
    fn should_show_fn(&self, state: &DeviceState) -> bool {
        state.definition().state == DefinitionState::Running
    }

    /// Called when the page is first opened, allows it to perform setup
    fn on_open_fn(&mut self, _: &mut DeviceState) {}

    /// Called when the page is closed, allows it to perform cleanup
    fn on_close_fn(&mut self, _: &mut DeviceState) {}

    /// Called at 30fps, allows periodic updates
    fn on_tick_fn(&mut self, _device: &mut DeviceState) -> Task<PageMessage> {
        Task::none()
    }

    /// Maps against an iced update() call for the page
    fn update_fn(&mut self, _device: &mut DeviceState, _message: PageMessage) -> Task<PageMessage> {
        Task::none()
    }

    /// Maps against an iced view() call for the page
    fn view_fn(&self, device: &DeviceState) -> Element<'_, PageMessage>;
}

// Page trait, so we can transparently handle different types, will map x_fn() -> x()
macro_rules! page_trait {
    ($trait_name:ident, $wrapper_name:ident, $state_type:ty, $variant:ident) => {
        pub(crate) trait $trait_name {
            fn icon(&self) -> &'static str {
                "error"
            }
            fn should_show(&self, state: &$state_type) -> bool {
                state.definition().state == DefinitionState::Running
            }

            fn on_open(&mut self, _state: &mut $state_type) {}
            fn on_close(&mut self, _state: &mut $state_type) {}
            fn on_tick(&mut self, _state: &mut $state_type) -> Task<PageMessage> {
                Task::none()
            }
            fn update(&mut self, _state: &mut $state_type, _msg: PageMessage) -> Task<PageMessage> {
                Task::none()
            }

            fn view(&self, state: &$state_type) -> Element<'_, PageMessage>;
        }

        pub(crate) struct $wrapper_name<T: $trait_name>(pub T);

        impl<T: $trait_name> Page for $wrapper_name<T> {
            fn icon(&self) -> &'static str {
                self.0.icon()
            }

            fn should_show_fn(&self, device: &DeviceState) -> bool {
                //use crate::devices::states::LoadState;

                // We shouldn't show anything if we're in an error state
                if matches!(device.definition().state, DefinitionState::Error(_)) {
                    return false;
                }

                let DeviceState::$variant(state) = device else {
                    unreachable!()
                };

                // We can get away with this because Audio and Control pages both contain
                // the same state structure for this. We should be more careful though!
                // TODO: Turn this on :D
                // if state.device_state.state == LoadState::Error {
                //     return false;
                // }

                self.0.should_show(state)
            }

            fn on_open_fn(&mut self, device: &mut DeviceState) {
                let DeviceState::$variant(state) = device else {
                    unreachable!()
                };
                self.0.on_open(state)
            }

            fn on_close_fn(&mut self, device: &mut DeviceState) {
                let DeviceState::$variant(state) = device else {
                    unreachable!()
                };
                self.0.on_close(state)
            }

            fn on_tick_fn(&mut self, device: &mut DeviceState) -> Task<PageMessage> {
                let DeviceState::$variant(state) = device else {
                    unreachable!()
                };
                self.0.on_tick(state)
            }

            fn update_fn(
                &mut self,
                device: &mut DeviceState,
                message: PageMessage,
            ) -> Task<PageMessage> {
                let DeviceState::$variant(state) = device else {
                    unreachable!()
                };

                self.0.update(state, message)
            }

            fn view_fn(&self, device: &DeviceState) -> Element<'_, PageMessage> {
                let DeviceState::$variant(state) = device else {
                    unreachable!()
                };

                self.0.view(state)
            }
        }
    };
}

page_trait!(AudioPage, AP, AudioState, Audio);
page_trait!(ControllerPage, CP, ControlState, Control);
