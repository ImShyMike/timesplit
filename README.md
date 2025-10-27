<p align="center">
  <a href="https://crates.io/crates/timesplit">
    <h1>timesplit</h1>
  </a>
</p>
<p align="center">The easy way to use multiple WakaTime compatible instances at once!</p>

<p align="center">
    <a href="https://github.com/ImShyMike/timesplit/actions/workflows/build.yaml"><img alt="GitHub Actions Workflow Status" src="https://img.shields.io/github/actions/workflow/status/ImShyMike/timesplit/build.yaml?style=flat-square"></a>
    <a href="https://crates.io/crates/timesplit"><img alt="GitHub last commit" src="https://img.shields.io/github/last-commit/ImShyMike/timesplit?style=flat-square&color=green"></a>
    <a href="https://crates.io/crates/timesplit"><img alt="Crates.io Version" src="https://img.shields.io/crates/v/timesplit?style=flat-square&color=yellow"></a>
    <a href="https://crates.io/crates/timesplit"><img alt="Crates.io License" src="https://img.shields.io/crates/l/timesplit?style=flat-square&color=orange"></a>
    <a href="https://crates.io/crates/timesplit"><img alt="Crates.io Size" src="https://img.shields.io/crates/size/timesplit?style=flat-square&color=red"></a>
</p>

---

## Installation

Prebuilt binaries can be found for all platforms in the [releases section](https://github.com/ImShyMike/timesplit/releases) of the repository.

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
curl -s -o install.sh https://raw.githubusercontent.com/ImShyMike/timesplit/refs/heads/main/install.sh
./install.sh
```

### Windows/MacOS

This will need manual installation for now :(
