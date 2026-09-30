# Runtime

This folder is the Ollama Windows program, not source code and not the model.

Download the Windows zip from https://github.com/ollama/ollama/releases and unzip all of it here.

Expected result:

```
runtime/
├── ollama.exe
└── lib/
    └── ollama/
```

`bin/ollama.exe` is also accepted if the rest of the zip is next to it.

Do not put the Ollama git repository here.
Do not put model blobs here.
Do not install Ollama into Program Files.
Do not change the host PATH.

Binaries in this folder are gitignored. This README is the only file that stays in git.
