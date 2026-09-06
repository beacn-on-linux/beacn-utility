use enum_map::Enum;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[cfg(not(target_arch = "wasm32"))]
use interprocess::local_socket::tokio::prelude::LocalSocketStream;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn get_pipeweaver_socket_path() -> anyhow::Result<PathBuf> {
    const PIPEWEAVER_APP_NAME: &str = "PipeWeaver";
    const PIPEWEAVER_APP_NAME_ID: &str = "pipeweaver";

    use directories::BaseDirs;
    use std::{env, fs};
    let path = BaseDirs::new()
        .and_then(|base| base.runtime_dir().map(|p| p.to_path_buf()))
        .map(Ok::<PathBuf, anyhow::Error>)
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

#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn send_json(stream: &mut LocalSocketStream, value: &Value) -> anyhow::Result<()> {
    let data = serde_json::to_vec(value)?;

    let len = u32::try_from(data.len())?;

    stream.write_u32(len).await?;
    stream.write_all(&data).await?;

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn read_json(stream: &mut LocalSocketStream) -> anyhow::Result<Value> {
    let len = stream.read_u32().await?;

    let mut data = vec![0u8; len as usize];
    stream.read_exact(&mut data).await?;

    Ok(serde_json::from_slice(&data)?)
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Enum, EnumIter, Serialize, Deserialize)]
pub enum Mix {
    #[default]
    A,
    B,
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Enum, EnumIter, Serialize, Deserialize)]
pub enum MuteTarget {
    #[default]
    TargetA,
    TargetB,
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Enum, EnumIter, Serialize, Deserialize)]
pub enum OrderGroup {
    #[default]
    Default,
    Pinned,
    Hidden,
}
