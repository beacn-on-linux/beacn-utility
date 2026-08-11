use crate::devices::manager::DefinitionState;
use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::devices::states::control::ControlState;
use crate::ui_iced::app::DeviceState;
use iced::{Element, Task};

#[derive(Debug, Clone)]
pub enum PageMessage {
    // ErrorPage(ErrorPageMessages),
    // ConfigPage(ConfigMessage), // Empty for now
}

pub trait Page {
    /// The icon to show in the left panel
    fn icon(&self) -> &'static str {
        "error"
    }

    /// Whether this page should be shown at all, defaults to Device = Working
    fn should_show_fn(&self, state: &DeviceState) -> bool {
        state.definition().state == DefinitionState::Running
    }

    /// Called when the page is first opened, allows it to perform setup
    fn on_open_fn(&mut self, state: &DeviceState) {}

    /// Called when the page is closed, allows it to perform cleanup
    fn on_close_fn(&mut self) {}

    /// Maps against an iced update() call for the page
    fn update_fn(&mut self, device: &mut DeviceState, message: PageMessage) -> Task<PageMessage>;

    /// Maps against an iced view() call for the page
    fn view_fn(&self, device: &DeviceState) -> Element<'_, PageMessage>;
}

// Page trait, so we can transparently handle different types, will map x_fn() -> x()
macro_rules! page_trait {
    ($trait_name:ident, $wrapper_name:ident, $state_type:ty, $variant:ident) => {
        pub trait $trait_name {
            fn icon(&self) -> &'static str {
                "error"
            }
            fn should_show(&self, state: &$state_type) -> bool {
                state.definition().state == DefinitionState::Running
            }

            fn on_open(&mut self, state: &$state_type) {}
            fn on_close(&mut self) {}

            fn update(
                &mut self,
                state: &mut $state_type,
                message: PageMessage,
            ) -> Task<PageMessage>;
            fn view(&self, state: &$state_type) -> Element<'_, PageMessage>;
        }

        pub struct $wrapper_name<T: $trait_name>(pub T);

        impl<T: $trait_name> Page for $wrapper_name<T> {
            fn icon(&self) -> &'static str {
                self.0.icon()
            }

            fn should_show_fn(&self, device: &DeviceState) -> bool {
                let DeviceState::$variant(state) = device else {
                    unreachable!()
                };
                self.0.should_show(state)
            }

            fn on_open_fn(&mut self, device: &DeviceState) {
                let DeviceState::$variant(state) = device else {
                    unreachable!()
                };
                self.0.on_open(state)
            }

            fn on_close_fn(&mut self) {
                self.0.on_close()
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
page_trait!(ControllerPage, CPW, ControlState, Control);
