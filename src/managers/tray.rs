use crate::{APP_NAME, APP_TITLE, ICON, ManagerMessages, WindowMessage, get_logs_path};
use anyhow::Result;
use beacn_lib::flume::{Receiver, Sender, bounded};
use image::GenericImageView;
use ksni::menu::StandardItem;
use ksni::{Category, Icon, MenuItem, Status, ToolTip, Tray, TrayMethods};
use log::{debug, warn};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{env, fs};

enum TrayMessages {
    Activate,
    OpenLogs,
    Quit,
}

pub async fn handle_tray(
    tray_manager: Receiver<ManagerMessages>,
    tray_main_tx: Sender<WindowMessage>,
) -> Result<()> {
    debug!("Spawning Tray");

    // Create a temporary directory to store the icon
    let tmp_file_dir = env::temp_dir().join(APP_NAME);
    if !tmp_file_dir.exists() {
        fs::create_dir_all(&tmp_file_dir)?;
    }

    // Write the icon out to the temporary path
    let tmp_file_path = tmp_file_dir.join(format!("{APP_NAME}.png"));
    if !tmp_file_path.exists() || fs::remove_file(&tmp_file_path).is_ok() {
        fs::write(&tmp_file_path, ICON)?;
    } else {
        warn!("Unable to remove existing icon, using whatever is already there..");
    }

    let (icon_tx, icon_rx) = bounded(20);
    let icon = TrayIcon::new(icon_tx, &tmp_file_path);
    let handle = icon
        .disable_dbus_name(ashpd::is_sandboxed())
        .assume_sni_available(true)
        .spawn()
        .await;

    let handle = match handle {
        Ok(handle) => handle,
        Err(e) => {
            fs::remove_file(&tmp_file_path)?;
            warn!("Unable to Spawn the Tray Handler: {}", e);
            return Ok(());
        }
    };

    loop {
        tokio::select! {
            msg = icon_rx.recv_async() => {
                match msg {
                    Ok(TrayMessages::Activate) => {
                        let _ = tray_main_tx.send(WindowMessage::OpenWindow);
                    }

                    Ok(TrayMessages::OpenLogs) => {
                        if let Ok(logs) = get_logs_path() {
                            let _ = open::that(logs);
                        }
                    }

                    Ok(TrayMessages::Quit) => {
                        let _ = tray_main_tx.send(WindowMessage::Quit);
                        break;
                    }

                    Err(e) => {
                        warn!("Icon receiver channel broken, bailing: {e}");
                        break;
                    }
                }
            }

            msg = tray_manager.recv_async() => {
                match msg {
                    Ok(ManagerMessages::Quit) => {
                        break;
                    }

                    Err(e) => {
                        warn!("Message Handler channel Broken, bailing: {e}");
                        break;
                    }
                }
            }
        }
    }

    debug!("Stopping Tray");
    if !handle.is_closed() {
        handle.shutdown();
    }

    // Remove the temporary icon file
    fs::remove_file(tmp_file_path)?;
    debug!("Tray Stopped");
    Ok(())
}

// TODO: The Icon may come back later.
#[allow(unused)]
struct TrayIcon {
    icon: PathBuf,
    tx: Sender<TrayMessages>,
}

impl TrayIcon {
    fn new(tx: Sender<TrayMessages>, icon: &Path) -> Self {
        Self {
            icon: icon.to_path_buf(),
            tx,
        }
    }
}

impl Tray for TrayIcon {
    fn id(&self) -> String {
        APP_NAME.to_string()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayMessages::Activate);
    }
    fn category(&self) -> Category {
        Category::Hardware
    }
    fn title(&self) -> String {
        APP_TITLE.to_string()
    }
    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        static TRAY_ICON: LazyLock<Icon> = LazyLock::new(|| {
            let img = image::load_from_memory_with_format(ICON, image::ImageFormat::Png)
                .expect("Unable to Load Image");

            let (width, height) = img.dimensions();
            let mut data = img.into_rgba8().into_vec();

            for pixel in data.as_chunks_mut::<4>().0 {
                pixel.rotate_right(1) // RGBA to ARGB
            }

            Icon {
                width: width as i32,
                height: height as i32,
                data,
            }
        });

        vec![TRAY_ICON.clone()]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: String::from(APP_TITLE),
            description: String::from("A Tool for Configuring Beacn Devices"),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: String::from("Show"),
                activate: Box::new(|this: &mut TrayIcon| {
                    let _ = this.tx.try_send(TrayMessages::Activate);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: String::from("Open Logs"),
                activate: Box::new(|this: &mut TrayIcon| {
                    let _ = this.tx.try_send(TrayMessages::OpenLogs);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: String::from("Quit"),
                activate: Box::new(|this: &mut TrayIcon| {
                    let _ = this.tx.try_send(TrayMessages::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
