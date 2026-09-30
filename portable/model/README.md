# Model

This folder is model data only. No program and no source code.

After `ollama pull qwen3:4b` (or another tag), copy the Ollama models directory into this folder. That is usually:

```
%USERPROFILE%\.ollama\models
```

Expected result:

```
model/
├── blobs/
└── manifests/
```

Default model name in Settings: `qwen3:4b`

Do not put `ollama.exe` here. That belongs in `runtime/`.

Model files are large and are not committed to git.
