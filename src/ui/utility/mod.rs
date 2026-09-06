pub mod pipewire;

pub fn open_url(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        open::that_detached(url)?;
    }
    #[cfg(target_arch = "wasm32")]
    {
        webbrowser::open(url)?;
    }
    Ok(())
}
