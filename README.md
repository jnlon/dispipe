# dispipe

Dispipe sends line-based input on named pipes to pre-configured Discord
channels. Effectively, it lets you make write-only discord bots using shell
script, or any arbitrary language, without needing a library!

## Building

Built with rustc 1.45.0, earlier versions may not work. Depends on `serenity`,
`nix`, and `rust-ini` packages.

Compile with: `$ cargo build --release`

You can find the binary at `target/release/dispipe`

## Usage

To start the bot:

`$ dispipe /path/to/config.ini`

Send a message to a configured Discord channel:

`$ echo "hello from dispipe!" > /var/dispipe/example.fifo`

## Configuration

```ini
; Config Structure
; 1. The 'Dispipe' section is mandatory, it specifies the root directory and discord bot token
; 2. All other sections are optional "fifo configs" that specify a fifo name and channel ID

; Example:

[Dispipe]                       ; Mandatory config section
token = ...                     ; Discord bot token
root = /var/dispipe             ; Directory containing FIFO files

                                ; Example "fifo config" section
[Example]                       ; Title used for easy identification (echoed to stdout with message)
fifo = example.fifo             ; Name of the fifo file to listen on
channel = 99999999999999999     ; Channel ID to send messages to

; ...                           ; Any number of additional fifo configs can be created
```

## Notes

**Pre-Requisites**

You must register a bot with discord, retrieve a token, and invite the bot to a
server. The server must also allow your bot the write messages permission.

**Usage**

- Messages are sent when a newline character '\n' is encountered on the configured FIFO.
- Lines longer than 2000 bytes will be truncated, 2000 bytes is the discord message limit.
- You can safely write to the FIFO from multiple processes.

## Furter Reading

- [discord.py - Creating a Bot Account](https://discordpy.readthedocs.io/en/latest/discord.html)
- `man 7 pipe`
