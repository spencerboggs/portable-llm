# PortableLLM

A local LLM that runs from a USB drive on Windows. The stick stores the app, Ollama runtime, model, and knowledge files. Plug it into a PC, load onto a drive with enough free space, chat, then remove the temporary host copy when you are done. It does not install into Program Files or change PATH, registry, or other system settings.

Do not put the whole git repo on the flash drive. Build on a normal PC, then copy only the finished app onto the USB.

## How space works

You need space in two places:

1. **USB stick** - app, runtime, model, and knowledge
2. **Host PC drive** (when you click Load Model) - temporary copy at `<drive>:\PortableLLM\`

That host copy is roughly the size of your runtime + model. If those take 5 GB on the USB, plan on about 5 GB free on the host drive. The Dashboard shows required vs available space before you load.

Choose a model that fits your USB. Bigger models need more USB space and about the same free space again on the host.

## Recommended model

**qwen3:4b** through Ollama is the default pick. It fits most flash drives and works for general use and coding help.

Any Ollama-compatible model works. Set **Model name** in Settings to match what you put in `model/`.

Models are not included in this repository.

## Part 1: Build on your PC

Do this on a development machine.

Requirements:

- Windows 10/11
- [Node.js](https://nodejs.org/) (includes npm)
- [Rust](https://rustup.rs/) (stable, MSVC)

### 1. Clone and install

```bash
git clone https://github.com/spencerboggs/portable-llm.git
cd portable-llm
npm install
```

### 2. Add the Ollama runtime

Get a Windows build from [Ollama releases](https://github.com/ollama/ollama/releases) and put the binary at:

```
portable/runtime/ollama.exe
```

`portable/runtime/bin/ollama.exe` also works.

### 3. Add a model

On a machine that already has Ollama:

```bash
ollama pull qwen3:4b
```

Copy that machine's Ollama models folder into:

```
portable/model/
```

You usually need the `blobs/` and `manifests/` folders for your model.

### 4. Edit knowledge (optional)

Swap the sample files in `portable/knowledge/` for your own:

| File | Purpose |
|------|---------|
| `profile.md` | Facts about you |
| `personality.md` | How the assistant should behave |
| `programming.md`, `cybersecurity.md`, `custom/` | Extra notes used in chat |

### 5. Build the app

```bash
npm run tauri build
```

The exe is usually at:

```
src-tauri/target/release/PortableLLM.exe
```

If Tauri also builds installers, ignore those. The flash drive needs the standalone `.exe`.

You can test on the PC with `npm run tauri dev` before you fill the USB.

## Part 2: Put it on the flash drive

Make a folder on the USB (for example `PortableLLM`) and copy only these:

| From your PC | Onto the USB |
|--------------|--------------|
| `src-tauri/target/release/PortableLLM.exe` | `PortableLLM.exe` (next to the folders below) |
| contents of `portable/runtime/` | `runtime/` |
| contents of `portable/model/` | `model/` |
| contents of `portable/knowledge/` | `knowledge/` |
| contents of `portable/config/` | `config/` |
| contents of `portable/data/` | `data/` |

Target layout:

```
E:\PortableLLM\          (letter depends on the PC)
├── PortableLLM.exe
├── runtime/
│   └── ollama.exe
├── model/
│   ├── blobs/
│   └── manifests/
├── knowledge/
│   ├── profile.md
│   ├── personality.md
│   └── ...
├── config/
└── data/
    ├── conversations/
    ├── scripts/
    └── logs/
```

Leave these off the USB:

- `src/` and `src-tauri/` (except the built `.exe`)
- `node_modules/`
- `.git/`
- everything else from the repo

That stuff is only for building.

### Run from the stick

1. Plug the USB into a Windows PC.
2. Open the folder and run `PortableLLM.exe`.
3. On the Dashboard, pick a host drive with enough free space.
4. Click **Load Model** and wait for **Running**.
5. Use Chat, Knowledge, Scripts, or Settings.
6. Click **Remove Model** when finished. That deletes the host copy. The USB stays intact.

**Portable Mode** (default) copies the model to the host so you can unplug the USB after loading. If you turn it off, keep the stick plugged in.

## Using the app

### Dashboard

1. Check CPU, RAM, and GPU info.
2. Select a host drive with enough free space.
3. Click **Load Model**.
4. When status is **Running**, open Chat.
5. Click **Remove Model** when done. That only deletes `<drive>:\PortableLLM\` on the host.

### Chat

- Enter sends. Shift+Enter makes a new line.
- Use Stop or Regenerate if you need them.
- Code blocks have Copy. Save sends a snippet to Scripts.
- Profile, personality, and matching knowledge files go into context. The model runs locally.

### Knowledge

Edit Markdown under `knowledge/` in the app or with any editor on the stick. Click **Reload** if you changed files outside the app.

### Scripts

Files land in `data/scripts/`. The app does not run them. Open the folder and run them yourself if you want.

### Settings

| Setting | Meaning |
|---------|---------|
| Model name | Ollama tag matching files in `model/` |
| Ollama port | Local port for this app (default `11435`) |
| Portable Mode | Copy model to host so the USB can be removed while running |
| Internet tools | Optional web tools. Chat still works offline. |

## Safety

This is meant to run on shared or other people's PCs. It will not:

- Change PATH, env vars, registry, services, or drivers
- Install into Program Files
- Change a global Ollama install
- Run generated scripts on its own
- Write outside `<drive>:\PortableLLM\` on the host

If you need PATH or install changes, do those yourself. The assistant can tell you what to run, but it will not do it.

## Repo layout (development)

```
portable-llm/
├── src/                 # React UI
├── src-tauri/           # Rust / Tauri backend
├── portable/            # assets you copy to the USB later
│   ├── runtime/
│   ├── model/
│   ├── knowledge/
│   ├── config/
│   └── data/
├── scripts/
└── docs/ARCHITECTURE.md
```

## License

MIT. See [LICENSE](LICENSE).
