# zodiac

[![Release](https://github.com/mmxkarimi/Zodiac/actions/workflows/release.yml/badge.svg)](https://github.com/mmxkarimi/Zodiac/actions/workflows/release.yml)

A small, fast MCP server for [Kiwix](https://www.kiwix.org/) (kiwix-serve). It lets AI tools search and read offline Wikipedia and other ZIM books.

It uses only two crates (`http` and `json`), and it talks to AI tools over standard input and output.

## Requirements

- Rust (edition 2024 or newer)
- A running `kiwix-serve` with at least one ZIM file

Start Kiwix, for example:

```sh
kiwix-serve --port=8080 wikipedia_en_all_mini_2026-09.zim
```

## Build

```sh
cargo build --release
```

The program is in `target/release/zodiac`.

## Run

By default, it connects to `http://127.0.0.1:8080`. You can change that in three ways:

```sh
zodiac                          # uses the default address
zodiac --port 9000              # local server on port 9000
zodiac --url http://127.0.0.1:8080   # any server address
KIWIX_URL=http://127.0.0.1:8080 zodiac
```

## Tools

The server gives AI tools these actions:

- `kiwix_list_books`: list the books on the server
- `kiwix_suggest`: suggest article titles for a word or phrase
- `kiwix_search`: search the books
- `kiwix_read_page`: read an article as Markdown
- `kiwix_random`: open a random article
- `kiwix_status`: check if the server is up

## Use with an AI tool

Add it to your MCP client config. For example:

```json
{
  "mcpServers": {
    "kiwix": {
      "command": "/path/to/zodiac",
      "args": ["--url", "http://127.0.0.1:8080"]
    }
  }
}
```

## License

GNU LESSER GENERAL PUBLIC LICENSE 3.0