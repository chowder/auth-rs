# Auth-RS

A linux native alternative to the Jagex Launcher, possibly also works on MacOS/Windows

## Installation
1. Download the latest release from [GitHub Releases](../../releases)
2. Extract the binary and place it in your `$PATH` (e.g., `/usr/local/bin/`)
3. Make it executable: `chmod +x auth-rs`

## Quick Start

### 1. Authenticate with Jagex
```bash
auth-rs authorize
```

### 2. List Available Characters
```bash
auth-rs ls
```
Example output:
```
• Character Display Name (ID: 123456789)
```

### 3. Launch Game Client
```bash
auth-rs exec --character-id 123456789 java -- -jar RuneLite.jar
```

## Desktop Integration

```ini
[Desktop Entry]
Name=RuneLite <insert character name>
Comment=Launch RuneLite
Exec=auth-rs exec --character-id YOUR_CHARACTER_ID java -- -jar /path/to/RuneLite.jar
Icon=runelite
Terminal=false
Type=Application
Categories=Game;
```

Save this as `~/.local/share/applications/runelite-jagex.desktop` and replace `YOUR_CHARACTER_ID` with your actual character ID.

### Using alternative clients

When using `exec`, the following environment variables are set for the launched process:

- `JX_SESSION_ID` - Jagex session ID
- `JX_CHARACTER_ID` - Selected character ID  
- `JX_DISPLAY_NAME` - Character display name

I'm assuming that all clients that support jagex accounts work the same way, so this launcher may also work for the official OSRS client and maybe even the RS3 client