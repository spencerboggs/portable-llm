# Architecture

## Idea

USB holds the app, portable Ollama, model, and knowledge. A host drive is only a temporary run location that uses that machine's CPU/GPU/RAM. Remove Model deletes the host folder.

## Safety

The app must not:

- Change PATH, registry, services, drivers, or global env vars
- Touch an existing Ollama install
- Auto-run generated scripts
- Scatter files outside `<drive>:\PortableLLM\`

The model may suggest manual system steps. It must not perform them.

## Stack

| Layer | Choice |
|-------|--------|
| Shell | Tauri 2 (Windows first) |
| UI | React, TypeScript, Tailwind |
| Runtime | Bundled portable Ollama |
| Model | Small Ollama model (default name: `qwen3:4b`) |
| Knowledge | Markdown + keyword chunk retrieval |
| Tools | App-mediated `web_search`, `fetch_url` when enabled |

## Layout

```
USB / portable root:
├── PortableLLM.exe   (or npm run tauri dev)
├── runtime/
├── model/
├── knowledge/
├── config/
└── data/

Host after Load Model:
<drive>:\PortableLLM\
├── runtime/
├── model/
├── knowledge-cache/
├── data/
├── temp/
└── manifest.json
```

## Backend modules

1. **paths** - Find USB/portable root without hard-coded drive letters
2. **drives** - Volumes, free space, SSD/HDD when Windows reports it
3. **hardware** - CPU, RAM, GPU, VRAM (info only; CPU inference always allowed)
4. **staging** - Copy runtime/model to host `PortableLLM\`, write manifest
5. **ollama** - Start/stop isolated Ollama (`OLLAMA_MODELS`, `OLLAMA_HOST`)
6. **chat** - Stream chat; build system context
7. **knowledge** - Chunk + keyword retrieval; profile/personality files
8. **conversations** - JSON under `data/conversations/`
9. **scripts** - Save scripts; no auto-run
10. **tools** - Optional web tools
11. **cleanup** - Stop Ollama; delete host folder only
12. **logging** - `data/logs/portablellm.log`

## Context order

```
App safety instructions
+ personality.md
+ profile.md
+ retrieved knowledge chunks
+ tool notes (if enabled)
+ conversation
```

## Modes

- **Portable Mode (default):** copy model to host so the USB can be removed
- **USB Required Mode:** keep model on USB; host mainly gets runtime

## Model packaging

Model files are not in git. Put them in `portable/model/` locally or as release assets outside the repo.
