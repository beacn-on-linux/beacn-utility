use crate::ui::pages::info_row;
use crate::{HASH, VERSION, has_autostart};
use anyhow::Result;
use iced::widget::{Space, checkbox, column, rule, text};
use iced::{Element, Task, window};
use log::{debug, warn};
use window::Id;

#[derive(Debug, Copy, Clone)]
pub(crate) enum SettingsMessage {
    EnableAutostart(bool),
    AutoStartChanged,
}

pub(crate) struct SettingsPage {
    autostart_enabled: Result<bool>,
}

impl SettingsPage {
    pub(crate) fn new() -> Self {
        Self {
            autostart_enabled: has_autostart(),
        }
    }

    pub(crate) fn update(&mut self, id: Id, message: SettingsMessage) -> Task<SettingsMessage> {
        debug!("Handling Message: {:?}", message);

        match message {
            SettingsMessage::EnableAutostart(e) => {
                return self.set_autostart(id, e);
            }
            SettingsMessage::AutoStartChanged => self.autostart_enabled = has_autostart(),
        }

        Task::none()
    }

    pub(crate) fn view(&self) -> Element<'_, SettingsMessage> {
        let title = "About the Beacn Utility";

        let version_text = format!("{}", VERSION);
        let hash_text = format!("{}", HASH);

        let autostart: Element<'_, SettingsMessage> =
            if let Ok(autostart_enabled) = self.autostart_enabled {
                checkbox(autostart_enabled)
                    .label("Auto-Start the Beacn Utility on Login")
                    .on_toggle(SettingsMessage::EnableAutostart)
                    .into()
            } else {
                Space::new().into()
            };

        let content = column![
            text(title).size(24),
            Space::new().height(10),
            rule::horizontal(1),
            Space::new().height(10),
            info_row("Version:", version_text),
            info_row("Revision:", hash_text),
            Space::new().height(10),
            rule::horizontal(1),
            Space::new().height(10),
            autostart,
        ]
        .spacing(8)
        .padding(20);

        Element::from(content).into()
    }

    #[cfg(not(target_os = "linux"))]
    fn set_autostart(&mut self, window_id: window::Id, enabled: bool) -> Task<SettingsMessage> {
        Task::none()
    }

    #[cfg(target_os = "linux")]
    fn set_autostart(&mut self, id: Id, enabled: bool) -> Task<SettingsMessage> {
        use crate::{APP_NAME, BACKGROUND_PARAM, get_autostart_file, run_async_blocking};
        use anyhow::anyhow;
        use ashpd::WindowIdentifier;
        use ashpd::desktop::background::Background;
        use ini::Ini;
        use std::{env, fs};

        if ashpd::is_sandboxed() {
            println!("Running inside Flatpak, using Background Portal");

            return window::run(id, move |window| {
                let window_handle = window.window_handle().unwrap().as_raw();
                let display_handle = window.display_handle().ok().map(|d| d.as_raw());

                // This needs to be blocked, the handles aren't safely sendable across threads
                // and this lookup is async, so we need to block here.
                run_async_blocking(WindowIdentifier::from_raw_handle(
                    &window_handle,
                    display_handle.as_ref(),
                ))
            })
            .then(move |identifier| {
                // We can send this directly into an iced task, rather than blocking
                Task::future(async move {
                    let reason = "Manage Beacn Devices on Startup";

                    let request = Background::request()
                        .identifier(identifier)
                        .reason(reason)
                        .auto_start(enabled)
                        .dbus_activatable(false)
                        .command::<Vec<_>, String>(vec![
                            String::from(APP_NAME),
                            String::from(BACKGROUND_PARAM),
                        ]);

                    debug!("Requesting Background Access");

                    match request.send().await {
                        Ok(r) => match r.response() {
                            Ok(r) => {
                                let c = r.auto_start();
                                if c != enabled {
                                    warn!("Failed to set autostart, expected {enabled}, got {c}");
                                }
                            }
                            Err(e) => warn!("Failed to request background access: {e}"),
                        },
                        Err(e) => warn!("Failed to request background access: {e}"),
                    }

                    SettingsMessage::AutoStartChanged
                })
            });
        }

        debug!("Running Outside Flatpak, manually handling");
        let result = match get_autostart_file() {
            Ok(path) => {
                if path.exists() && fs::remove_file(path.clone()).is_err() {
                    Err(anyhow!("Unable to remove existing AutoStart"))
                } else if enabled {
                    if let Ok(exe) = env::current_exe() {
                        let mut conf = Ini::new();
                        let exe = exe.to_string_lossy().to_string();

                        conf.with_section(Some("Desktop Entry"))
                            .set("Type", "Application")
                            .set("Name", "Beacn Utility")
                            .set("Comment", "A Tool for Configuring Beacn Devices")
                            .set("Exec", format!("{exe:?} {BACKGROUND_PARAM}"))
                            .set("Terminal", "false");

                        match conf.write_to_file(path) {
                            Ok(()) => Ok(()),
                            Err(e) => Err(anyhow!("Failed to Write File, {}", e)),
                        }
                    } else {
                        Err(anyhow!("Unable to Determine Executable"))
                    }
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(anyhow!(e)),
        };

        if let Err(e) = result {
            warn!("Failed to set autostart: {e}");
        }

        Task::done(SettingsMessage::AutoStartChanged)
    }
}
