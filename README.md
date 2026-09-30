# PortableLLM

Run a local LLM from a USB drive on Windows. The stick holds the app, Ollama runtime, model, and your knowledge files. When you plug it into another PC, you pick a drive with enough free space, load a temporary copy there, chat, then remove it. Nothing stays installed in Program Files, and the app does not change PATH, registry, or other system settings.

## How space works

Two places need disk space:

1. **USB stick** - holds the app, runtime, model, and knowledge for as long as you keep them there.
2. **Host PC drive** (Load Model) - gets a temporary copy under `<drive>:\PortableLLM\`. That copy is about as large as the runtime + model you put on the USB (plus a small buffer).

Rule of thumb: if the model + runtime take 5 GB on the USB, you need roughly 5 GB free on the host drive you select. The Dashboard shows required vs available space before you load.

Pick a model that fits the USB you plan to use. Larger models need more USB space and the same amount again (temporarily) on the host.

## Recommended model

**qwen3:4b** (via Ollama) is the default recommendation. It is small enough for typical flash drives and usable for general and programming help.

You can use any Ollama-compatible model. Change the model name in Settings to match what you placed under `portable/model/`.

Models are **not** included in this repository. You add them locally after cloning.

## Clone and set up

### Requirements

- Windows 10/11
- [Node.js](https://nodejs.org/) (npm included)
- [Rust](https://rustup.rs/) (stable, MSVC toolchain)
- A USB drive with enough free space for the app + runtime + model

### 1. Clone

```bash
git clone https://github.com/spencerboggs/portable-llm.git
cd portable-llm
npm install
```

### 2. Add the Ollama runtime

Download a Windows Ollama build from [Ollama releases](https://github.com/ollama/ollama/releases) and place the binary so one of these paths exists:

```
portable/runtime/ollama.exe
```

or

```
portable/runtime/bin/ollama.exe
```

See `portable/runtime/README.md`.

### 3. Add a model

Pull a model with Ollama on any machine, then copy that machine's Ollama models directory contents into:

```
portable/model/
```

You typically need `blobs/` and `manifests/` for the model you want.

Example with the recommended model (on a machine that already has Ollama):

```bash
ollama pull qwen3:4b
```

Then copy the models folder into `portable/model/`. Set **Model name** in Settings to `qwen3:4b` (or whatever tag you pulled).

### 4. Edit example knowledge (optional but useful)

Replace the sample files under `portable/knowledge/` with your own:

| File | Purpose |
|------|---------|
| `profile.md` | Facts about you (name, languages, interests) |
| `personality.md` | How the assistant should talk and behave |
| `programming.md`, `cybersecurity.md`, `custom/` | Extra notes the chat can retrieve |

### 5. Run

```bash
npm run tauri dev
```

To build a release binary:

```bash
npm run tauri build
```

Put the built app next to the `portable/` folder on the USB (or ship a layout where the exe sits beside `runtime/`, `model/`, `knowledge/`, and `data/`).

## Using the app

### Dashboard

1. Check CPU / RAM / GPU info.
2. Select a host drive with enough free space.
3. Click **Load Model**. Progress shows while files copy and Ollama starts.
4. When status is **Running**, go to Chat.
5. When finished, click **Remove Model**. That stops Ollama and deletes only `<drive>:\PortableLLM\`. The USB is left alone.

### Chat

- Type a message and press Enter (Shift+Enter for a new line).
- Stop generation if a reply is taking too long.
- Regenerate the last assistant reply if needed.
- Code blocks have Copy; use Save to send a snippet to the Scripts page.
- Chat uses `profile.md`, `personality.md`, and matching knowledge chunks. Inference stays on the local Ollama process.

### Knowledge

Open the **Knowledge** page to browse, edit, add, or delete Markdown files under `portable/knowledge/`.

- Edit `profile.md` and `personality.md` for who you are and how the assistant should act.
- Add notes under `custom/` or other folders for project-specific context.
- Use **Reload** after bulk file changes outside the app so the search index updates.

### Scripts

The **Scripts** page stores text you save from chat (or paste yourself). Files live under `portable/data/scripts/`.

PortableLLM does **not** run scripts for you. Copy or open the folder and run them yourself if you want.

### Settings

| Setting | Meaning |
|---------|---------|
| Model name | Ollama model tag (must match what is in `portable/model/`) |
| Ollama port | Isolated local port (default `11435`, avoids clashing with a normal Ollama on `11434`) |
| Portable Mode | On by default: copy model to the host so you can unplug the USB while running |
| Internet tools | Off by default. When on, the app can run controlled `web_search` / `fetch_url` for the model. Chat still works offline. |

## Safety

PortableLLM is meant for use on other people's machines as well as your own. It will not:

- Change PATH, env vars, registry, services, or drivers
- Install into Program Files
- Alter a global Ollama install
- Auto-execute generated scripts
- Write outside `<drive>:\PortableLLM\` on the host (aside from normal OS temp behavior tracked for cleanup)

If PATH or install steps would help, the assistant can describe them for you to do by hand.

## Project layout

```
portable-llm/
├── src/                 # React UI
├── src-tauri/           # Rust / Tauri backend
├── portable/
│   ├── runtime/         # ollama.exe (you provide)
│   ├── model/           # Ollama model files (you provide; not in git)
│   ├── knowledge/       # profile, personality, notes
│   ├── config/
│   └── data/            # conversations, scripts, logs, settings
├── scripts/             # helper scripts
└── docs/ARCHITECTURE.md
```

## License

MIT. See [LICENSE](LICENSE).
