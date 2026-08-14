use crate::integrations::pipeweaver::{PIPEWEAVER_APP_NAME, PIPEWEAVER_APP_NAME_ID};
use anyhow::Error;
use directories::BaseDirs;
use enum_map::Enum;
use interprocess::local_socket::tokio::prelude::LocalSocketStream;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::{env, fs};
use strum_macros::{Display, EnumIter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) fn get_pipeweaver_socket_path() -> anyhow::Result<PathBuf> {
    let path = BaseDirs::new()
        .and_then(|base| base.runtime_dir().map(|p| p.to_path_buf()))
        .map(Ok::<PathBuf, Error>)
        .unwrap_or_else(|| {
            let tmp_dir = env::temp_dir().join(PIPEWEAVER_APP_NAME);
            if !tmp_dir.exists() {
                fs::create_dir_all(&tmp_dir)?;
            }
            Ok(tmp_dir)
        })?;

    let socket_path = path.join(format!("{}.socket", PIPEWEAVER_APP_NAME_ID));
    Ok(socket_path)
}

pub(super) async fn send_json(stream: &mut LocalSocketStream, value: &Value) -> anyhow::Result<()> {
    let data = serde_json::to_vec(value)?;

    let len = u32::try_from(data.len())?;

    stream.write_u32(len).await?;
    stream.write_all(&data).await?;

    Ok(())
}

pub(super) async fn read_json(stream: &mut LocalSocketStream) -> anyhow::Result<Value> {
    let len = stream.read_u32().await?;

    let mut data = vec![0u8; len as usize];
    stream.read_exact(&mut data).await?;

    Ok(serde_json::from_slice(&data)?)
}

#[derive(Default, Debug, Copy, Clone, Hash, Eq, PartialEq, Enum, Display, EnumIter, Serialize)]
pub enum Mix {
    #[default]
    A,
    B,
}

#[derive(Default, Debug, Copy, Clone, Hash, Eq, PartialEq, Enum, Display, EnumIter, Serialize)]
pub enum MuteTarget {
    #[default]
    TargetA,
    TargetB,
}

#[derive(Default, Debug, Copy, Clone, Hash, Eq, PartialEq, Enum, Display, EnumIter, Serialize)]
pub enum OrderGroup {
    #[default]
    Default,
    Pinned,
    Hidden,
}

#[derive(Default, Debug, Copy, Clone, Hash, Eq, PartialEq, Enum, Display, EnumIter, Serialize)]
pub enum MuteState {
    #[default]
    Unmuted,
    Muted,
}
