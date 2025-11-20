<p align="center"><a href="https://crates.io/crates/timesplit"><img src="https://raw.githubusercontent.com/ImShyMike/timesplit/refs/heads/main/assets/text.svg" alt="timesplit"></a></p>

<p align="center">The easy way to use multiple WakaTime compatible instances at once!</p>

<p align="center">
    <a href="https://crates.io/crates/timesplit"><img alt="Crates.io Total Downloads" src="https://img.shields.io/crates/d/timesplit?style=flat-square"></a>
    <a href="https://github.com/ImShyMike/timesplit/actions/workflows/build.yaml"><img alt="GitHub Actions Workflow Status" src="https://img.shields.io/github/actions/workflow/status/ImShyMike/timesplit/build.yaml?style=flat-square"></a>
    <a href="https://crates.io/crates/timesplit"><img alt="GitHub last commit" src="https://img.shields.io/github/last-commit/ImShyMike/timesplit?style=flat-square&color=green"></a>
    <a href="https://crates.io/crates/timesplit"><img alt="Crates.io Version" src="https://img.shields.io/crates/v/timesplit?style=flat-square&color=yellow"></a>
    <a href="https://crates.io/crates/timesplit"><img alt="Crates.io License" src="https://img.shields.io/crates/l/timesplit?style=flat-square&color=orange"></a>
    <a href="https://crates.io/crates/timesplit"><img alt="Crates.io Size" src="https://img.shields.io/crates/size/timesplit?style=flat-square&color=red"></a>
</p>

---

## Installation

Prebuilt binaries can be found for all platforms in the [releases section](https://github.com/ImShyMike/timesplit/releases) of the repository.

You can also install the app from cargo with `cargo install timesplit`.

## Quick installation

The installation manager scripts will download the latest version and keep `timesplit run` always running in the background. They can also uninstall the program, check the installation status and update the installed version.

### Linux

#### Requirements

- curl
- systemctl

One liner:

```bash
curl -fsSL https://raw.githubusercontent.com/ImShyMike/timesplit/refs/heads/main/install.sh | sudo bash -s -- update
```

Download the install script and run it:

```bash
$ curl -o install.sh https://raw.githubusercontent.com/ImShyMike/timesplit/refs/heads/main/install.sh
$ ./install.sh
Usage: ./install.sh [COMMAND]

Commands:
    install     Install timesplit and set up autorun (requires sudo)
    uninstall   Remove timesplit and stop autorun (requires sudo)
    update      Update timesplit to the latest version (requires sudo)
    status      Check installation and service status
    help        Show this help message
```

### Windows

One liner: (needs an elevated PowerShell window)

```powershell
iwr -useb https://raw.githubusercontent.com/ImShyMike/timesplit/refs/heads/main/install.ps1 -OutFile install.ps1; powershell -ExecutionPolicy Bypass -Command ".\install.ps1 update"
```

### macOS

One liner:

```bash
curl -fsSL https://raw.githubusercontent.com/ImShyMike/timesplit/refs/heads/main/install_macos.sh | sudo bash -s -- update
```

Download the install script and run it:

```bash
$ curl -o install_macos.sh https://raw.githubusercontent.com/ImShyMike/timesplit/refs/heads/main/install_macos.sh
$ sudo ./install_macos.sh
Usage: ./install_macos.sh [COMMAND]

Commands:
  install     Install timesplit and set up autorun (requires sudo)
  uninstall   Remove timesplit and stop autorun (requires sudo)
  update      Update timesplit to the latest version (requires sudo)
  status      Check installation and service status
  help        Show this help message
```

## Usage/Setup

### Automatic

After installing run the following command to automatically add `timesplit` to your WakaTime config file and set `timesplit`'s main server to the one previously in the WakaTime config file.

```bash
timesplit setup
```

### Manual

In your `~/.wakatime.cfg` file, set the API url to `timesplit`'s addresss.

```cfg
[settings]
api_key = 39949664-5a5f-4c7d-95b2-44a864f67b6a
api_url = http://localhost:25893
```

_(This snippet uses `timesplit`'s default port.)_

> [!WARNING]  
> The api key must be a valid UUID to avoid compatibility issues!

## Configuration

The configuration file can be found in your home directory at `~/.timesplit.toml`.

### Quick config

```bash
$ timesplit config
Change the configuration

Usage:

Commands:
  list    List configured servers
  add     Add a new server
  remove  Remove a server by its index (use `config list` to find indexes)
  main    Make a server the main server by its index
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

> [!WARNING]  
> You may need to run the install script again after configuring the app! (using the `update` command)

## Known compatible backends

This program is know to be compatible with the following backend servers:

- [WakaTime](https://github.com/wakatime)
- [Wakapi](https://github.com/muety/wakapi)
- [Rustytime](https://github.com/ImShyMike/rustytime) (you should check this one out ;))
- [Hackatime](https://github.com/hackclub/hackatime)
- [Hackatime (old)](https://github.com/hackclub/archived-hacktime)
- [OtterTime](https://github.com/SkyfallWasTaken/ottertime)

All of the above servers were tested using the VS Code extension.

Others will likely work, this is just a list of verified working servers.

## Issues

Please feel free to [open an issue](https://github.com/ImShyMike/timesplit/issues/new) on the github if you come across a bug.

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=imshymike/timesplit&type=Timeline)](https://www.star-history.com/#imshymike/timesplit&Timeline)
