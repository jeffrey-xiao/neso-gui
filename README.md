# neso-gui

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Build Status](https://travis-ci.org/jeffrey-xiao/neso-gui.svg?branch=master)](https://travis-ci.org/jeffrey-xiao/neso-gui)

![Screenshot of Castlevania II: Simon's Quest](examples/screenshot.png)

An SDL2 interface to [`neos-rs`](https://gitlab.com/jeffrey-xiao/neso-rs).

## Features

 - Save file and save state support.
 - Debug views for nametables, pattern tables, colors, and palette.

## Usage

```
neso-gui 0.1.0
Jeffrey Xiao <jeffrey.xiao1998@gmail.com>
A NES emulator built with Rust and sdl2.

USAGE:
    neso-gui [FLAGS] [OPTIONS] <rom-path>

FLAGS:
    -d, --debug      Enable debug views.
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --config <config>    Path to configuration file.
    -f, --frames <frames>    Number of frames to run.

ARGS:
    <rom-path>    Path to rom.
```

## Configuration

When `neso-gui` is started, it looks for a configuration file in the following order:

1. The path specified by the `-c/--config` argument.
2. `$XDG_CONFIG_HOME/neso-gui/neso-gui.toml` if `$XDG_CONFIG_HOME` is set.
3. `$HOME/.config/neso-gui/neso-gui.toml`

## License

`nes-gui` is distributed under the terms of both the MIT License and the Apache License (Version
2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for more details.
