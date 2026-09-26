# Run the Utility in a Browser?

**NOTE**: This is glitchy as all hell, doesn't work properly in some cases, and was made entirely for the reasons of
"because I can". This is not a serious attempt at having a web browser as a host for beacn hardware.

Anyway, with that out the way. This folder contains helpers for having a web browser as a host for beacn hardware.

Compiling and running:

### Setup

Ensure that the `wasm32-unknown-unknown` rust target is installed:

```
rustup target add wasm32-unknown-unknown
```

Ensure that `wasm-bindgen` is installed:

```
cargo install wasm-bindgen-cli
```

### Compiling

Simply build the project with the wasm target, then run wasm-bindgen to generate the bindings

```
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/ target/wasm32-unknown-unknown/release/beacn-utility.wasm
```

### Running

Because we're using WebUSB, this has to be served from a web server, easiest way to do that is simply to enter the web
directory and run:

```
python3 -m http.server -p 8080
```

Then navigate to `http://localhost:8080` in your browser.

# Notes

- This *ONLY* works in Chromium Based Browsers.
    - Firefox considers WebUSB to be 'harmful', and I don't disagree
    - Safari is also opposed to the WebUSB spec.
- Devices occasionally randomly fail to open, no idea why, probably won't fix.
- Refreshing Beacn Link devices will lock up the tab.
- Pipeweaver support works, but is disabled because it takes too long to load.
    - Browsers can't cache the generated images, so they have to be re-generated every page load.
    - It takes about 25 seconds to generate said images best case.
- The EQ Widgets do not respond to touch events.
    - Yes, this *CAN* work on Android phones with devices directly attached.
    - No, this does NOT work well in those cases.