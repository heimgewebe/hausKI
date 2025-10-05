# Whisper ASR models

Store Whisper checkpoints (e.g. `ggml-base.en.bin`) inside sub-directories
named after the model variant you intend to run. Example layout:

```
models/
  whisper/
    base.en/
      ggml-base.en.bin
```

Model binaries stay untracked; this README keeps the directory present in the
repository and explains the expectation for local setups.
