# Fork model catalog extensions

This fork reads `$CODEX_HOME/models.extend.json` when creating a local model
manager (normally `~/.codex/models.extend.json`). No file means upstream behavior.
Restart Codex after editing it. The fork never modifies the file, the bundled
`models.json`, or the provider's cache with extension data.

The format is `{"models": [ ... ]}`. Entries match an **exact, case-sensitive
`slug`**. Existing models retain fields omitted from the extension. Explicit
top-level fields replace their old values; arrays and nested objects replace as
whole values, not recursive patches. New slugs append complete `ModelInfo`
entries. New entries must explicitly declare `context_window` and
`input_modalities`; use the actual service's documented capabilities, not GPT
defaults. URL and authentication remain in TOML provider configuration.

For example, to change only a known model's label:

```json
{
  "models": [
    {"slug": "existing-model-id", "display_name": "My local label"}
  ]
}
```

Replace `existing-model-id` with a slug already present in the active catalog.
For a new model, start with a complete catalog entry and adjust its model ID,
instructions, context limits, reasoning settings, tools, and input modalities.
Copying a GPT entry unchanged does not make another model support GPT features.

Extensions apply after the active provider catalog (including an explicit
`model_catalog_json` replacement), and are reapplied after remote refreshes.
Both model listing and metadata lookup see the same merged catalog. Upstream
default-model selection is retained; explicitly selecting a valid added model
is supported. A partial patch cannot resurrect a model removed from the active
catalog: it must then satisfy the full new-entry schema.

The limit is 4 MiB and 256 entries. Duplicate or empty slugs, malformed JSON,
invalid metadata, or invalid context limits cause a warning and leave the
unextended catalog intact; a bad entry never partially applies the file.
Hosted/cache-only managers without a local CODEX_HOME do not read this file.

This is metadata customization, **not protocol emulation or a minimum-client-
version bypass**. A model requiring a newer Codex protocol still requires an
upgrade. Server authorization, API compatibility, and provider authentication
continue to apply. Keep this feature separate from subagent provider overrides.
