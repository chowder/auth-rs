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

The easiest way to create a desktop entry is using the built-in command:

```bash
auth-rs create-desktop-entry --name "RuneLite (auth-rs)" --character-id 123456789 java -- -jar /path/to/RuneLite.jar
```

This creates a shortcut called "RuneLite (auth-rs)" that you can launch from your applications menu or by searching for it.

## SteamDeck / Steam

* Add a Game > Add a Non-Steam game
* If you already have a desktop entry for RuneLite (or any other client), you should be able to add this as a non-steam game
* If you do not have a desktop entry, add any application or locate the "auth-rs" binary on your system, then change properties after:
  * target: `"auth-rs"`
  * launch options: `"exec" "--character-id" "123456789" "java" "--" "-jar" "/path/to/RuneLite.jar"`

### Using alternative clients

When using `exec`, the following environment variables are set for the launched process:

- `JX_SESSION_ID` - Jagex session ID
- `JX_CHARACTER_ID` - Selected character ID  
- `JX_DISPLAY_NAME` - Character display name

I'm assuming that all clients that support jagex accounts work the same way, so this launcher may also work for the official OSRS client and maybe even the RS3 client