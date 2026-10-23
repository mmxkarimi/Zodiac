<div align="center">

# Zodiac

**A small, fast MCP server for ZIM archives and offline Wikipedia.**

[![Release](https://github.com/mmxkarimi/Zodiac/actions/workflows/release.yml/badge.svg)](https://github.com/mmxkarimi/Zodiac/actions/workflows/release.yml)
[![License: LGPL v3](https://img.shields.io/badge/License-LGPL%20v3-blue.svg)](https://www.gnu.org/licenses/lgpl-3.0)
![Rust](https://img.shields.io/badge/Rust-2024%20edition-orange?logo=rust)
![MCP](https://img.shields.io/badge/Protocol-MCP-6f42c1)

</div>

---

## Overview

Zodiac is an embedded [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) server for [ZIM archives](https://openzim.org/). It lets AI tools search and read offline Wikipedia (and other ZIM content) with no internet and no extra services.

It opens `.zim` files straight from disk using memory mapping (`memmap`). You do **not** need `kiwix-serve` or any other background program.

## Features

| | |
|---|---|
| **Embedded ZIM reader** | No outside servers or daemons. One binary does the job. |
| **Instant startup** | Opens gigabyte-sized ZIM files in under 1 ms with memory mapping. |
| **Microsecond lookups** | Binary search over sorted ZIM directory lists finds title prefixes in under 100 µs. |
| **Multi-book library** | Load one or many archives at once with repeated `--zim` flags. |
| **HTML to Markdown** | Turns Wikipedia HTML into clean Markdown that is easy for LLMs to read. |
| **Standard MCP over stdio** | Works with any MCP client that can start a local program. |

## Quick Start

```sh
# 1. Build
cargo build --release

# 2. Run with a ZIM file
./target/release/zodiac --zim /path/to/wikipedia.zim
```

Then [add Zodiac to your MCP client](#client-configuration).

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) with the 2024 edition (Rust 1.85 or newer)
- One or more `.zim` files, for example from the [Kiwix ZIM dumps](https://dumps.wikimedia.org/other/kiwix/zim/)

## Installation

### Build from source

```sh
git clone https://github.com/mmxkarimi/Zodiac.git
cd Zodiac
cargo build --release
```

The binary will be at `target/release/zodiac` (`zodiac.exe` on Windows).

### Prebuilt binaries

Release builds are made by GitHub Actions. Check the [Releases page](https://github.com/mmxkarimi/Zodiac/releases) for downloads.

## Usage

Give Zodiac one or more ZIM archives with the `--zim` flag, or as plain arguments:

```sh
# One archive
zodiac --zim /path/to/wikipedia.zim

# Many archives in one library
zodiac --zim /path/to/wikipedia.zim --zim /path/to/wiktionary.zim

# Positional arguments
zodiac /path/to/wikipedia.zim /path/to/stackexchange.zim

# Environment variable (separate paths with ";")
ZIM_PATH="/path/to/wiki1.zim;/path/to/wiki2.zim" zodiac
```

### Input options

| Method | Example | Notes |
|---|---|---|
| `--zim` flag | `--zim book.zim` | Repeat the flag for more books. |
| Positional | `zodiac a.zim b.zim` | Handy for quick runs. |
| `ZIM_PATH` | `ZIM_PATH="a.zim;b.zim"` | Separate paths with `;`. |

## Available MCP Tools

Zodiac gives these tools to MCP clients:

| Tool | What it does |
|---|---|
| `zim_list_books` | Lists all loaded archives with ID, title, language, article count, and file size. |
| `zim_suggest` | Gives instant prefix suggestions (autocomplete) for a term. |
| `zim_search` | Searches articles across all loaded archives and returns snippets. |
| `zim_read_page` | Reads an article as Markdown (or raw HTML). |
| `zim_random` | Returns a random article as Markdown. |
| `zim_status` | Shows reader status, latency, and the total number of loaded articles. |

### Example prompts

Once connected, you can ask your AI client things like:

- "List the books you have loaded."
- "Suggest article titles that start with *Alan Tur*."
- "Search the offline Wikipedia for *photosynthesis* and summarize the best match."
- "Show me a random article."

## Client Configuration

Add Zodiac to the MCP settings of your client (Claude Desktop, Codex, Antigravity, and others):

```json
{
  "mcpServers": {
    "zodiac": {
      "command": "/path/to/zodiac",
      "args": [
        "--zim",
        "/path/to/wikipedia.zim"
      ]
    }
  }
}
```

For Claude Desktop, this file is usually here:

| OS | Config file |
|---|---|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |

On Windows, use double backslashes (or forward slashes) in JSON paths, for example `"C:\\tools\\zodiac.exe"`.

Restart your client after you save the file.

## Contributing

Issues and pull requests are welcome.

1. Fork the repository
2. Create a branch: `git checkout -b my-change`
3. Make your change and double check it.
4. Open a pull request that explains what you changed and why

## License

Licensed under the [GNU Lesser General Public License v3.0](https://www.gnu.org/licenses/lgpl-3.0).