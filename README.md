# Beacn Utility for Linux

A UI for configuring and using the Beacn hardware on Linux. Join us on [Discord](https://discord.gg/PdsscuEhMh).

***

### USE AT YOUR OWN RISK

This code directly modifies the on-board storage of the Beacn Mic and Studio. While it's been tested and made to
be as safe as is possible, it was derived from reverse engineering and thus may not be accurate.

This project is not supported by, or affiliated in any way with Beacn. For official Beacn software, please refer
to their website.

In addition, this project accepts no responsibility or liability for any use of this software, or any problems
which may occur from its use. Please read the LICENSE for more information.

***
# Installation

## Automatic Installation
The Beacn Utility project provides a simple script which will attempt to install the correct package for your
distribution. This script will install the utility in such a way that future releases will be managed through your
standard package manager.

There is a [wiki page](https://github.com/beacn-on-linux/beacn-utility/wiki/Release-Transparency) which describes the
process, and how you can verify that the code you're running is the same as what is in this repository.

Run the following in a Terminal, and follow the prompts:
```bash
curl -fsSL https://beacn-on-linux.github.io/beacn-utility-repo/scripts/install.sh | bash
```

## Manual Installation
If you don't want to use the script, there are still options available! The [releases page](https://github.com/beacn-on-linux/beacn-utility/releases/latest) 
contains the following:

* `.rpm` package for Redhat based distributions (Fedora, CentOS, RHEL, etc)
* `.deb` package for Debian based distributions (Debian, Ubuntu, Linux Mint, Pop, etc)
* `.flatpakref` A reference file for the Beacn Utility Flatpak repository
* Compile from Source (instructions below)

### Notes
 * The Beacn Utility is also available in the AUR as `beacn-utility`
 * The RPM and DEB packages do not provide automatic updates, and there's no app check.
 * For Bazzite, you can install the rpm via ostree, although I'd recommend the flatpak instead.

***
![img.png](.github/resources/img.png)

Currently, this tool is quite barebones and basic and is likely going to stay that way, it's primary goal is to provide
a way to adjust the DSP values of the Mic or Studio. Outside of rendering a test image, Mix and Mix Create support is
mostly absent. This app is still quite new, so may also might be slightly buggy, expect issues.
***

## Getting Started

### Setting Up Beacn Devices on Linux

The steps taken to set up Beacn Devices depend on the current package version of your distribution.

If you are running a `systemd` version older than 257.7, perform the following steps:
1) Copy `50-beacn.rules` from this repository to `/etc/udev/rules.d/`
2) Run `sudo udevadm control --reload-rules && sudo udevadm trigger` 

If you are running an `alsa-ucm-conf` version older than 1.2.15, and are using a Beacn Mic or Beacn Studio:
1) Manually install the [ALSA UCM profiles](https://github.com/beacn-on-linux/beacn-ucm-profiles) for the Beacn Hardware.

If you've needed to perform any of the above, unplug and replug your beacn device. For the Mic and Studio, you should
now see properly allocated Microphone / Headphone channels in your audio settings.
***

## Compiling From Source

If you simply want to just run this app, you can do so with the following:

1) Check out this repository
2) Run `cargo run --release`

### Building this App

If you instead want to build the app and have a useful binary you can link to:

1) Check out this repository
2) Run `cargo build --release`
3) Grab `target/release/beacn-utility`

***

## Compiling to Flatpak

To build a local flatpak of this project, check out the [beacn-utility-flatpak](https://github.com/beacn-on-linux/beacn-utility-flatpak)
repository.

***
## Current Project Status

Not Yet Implemented:
* Beacn Mix and Mix Create support (Need to make a mixer)
* Probably a button or two, let me know if you spot one.

Not (currently) Planned:
* Profiles
* Audio Visualisations

***

This tool may eventually be merged into a 'Bigger' app that includes other Beacn devices, this will happen as and
when time permits and devices are handled.
