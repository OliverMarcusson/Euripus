# Admin Search Rule JSON Import

Use the `Add JSON` button in the admin search panel to batch-create search rule groups from a JSON array.

## Canonical Format

The import payload is a top-level JSON array where each item becomes one pattern group.

```json
[
  {
    "kind": "country",
    "value": "se",
    "matchTarget": "channel_or_category",
    "matchMode": "prefix",
    "priority": 10,
    "enabled": true,
    "patterns": ["SE:", "SE|", "SWE|", "SWEDEN"]
  }
]
```

## Fields

- `kind`: required. One of `country`, `provider`, `flag`.
- `value`: required. The normalized output value that should be stored.
- `matchTarget`: required. One of:
  - `channel_name`
  - `category_name`
  - `program_title`
  - `channel_or_category`
  - `any_text`
- `matchMode`: required. One of `prefix`, `contains`, `exact`.
- `patterns`: required in the canonical format. An array of strings. Each string becomes one stored pattern row.
- `countryCodes`: required for `provider` groups. An array of canonical country codes such as `["se", "uk"]`.
- `priority`: optional. Defaults to `0`.
- `enabled`: optional. Defaults to `true`.

## Defaults And Validation

- The entire import is all-or-nothing. If any item is invalid, nothing is created.
- Patterns are trimmed and deduplicated the same way as manual admin entry.
- `patterns` must contain at least one non-empty string.
- `value` must be a non-empty string.
- `provider` groups must include at least one valid `countryCodes` entry that matches either:
  - an already existing country rule value, or
  - a `country` rule included in the same JSON import batch.
- Pattern group enum fields must match the allowed values exactly.

## Examples

### Sweden Country Prefixes

```json
[
  {
    "kind": "country",
    "value": "se",
    "matchTarget": "channel_or_category",
    "matchMode": "prefix",
    "priority": 10,
    "enabled": true,
    "patterns": ["SE:", "SE|", "SWE|", "SWEDEN"]
  }
]
```

### Viaplay Provider Aliases

```json
[
  {
    "kind": "provider",
    "value": "viaplay",
    "matchTarget": "channel_or_category",
    "matchMode": "contains",
    "priority": 10,
    "enabled": true,
    "countryCodes": ["se", "dk", "no"],
    "patterns": ["VIAPLAY", "Viaplay SE", "V PLAY"]
  }
]
```

### PPV And VIP Flags

```json
[
  {
    "kind": "flag",
    "value": "ppv",
    "matchTarget": "channel_or_category",
    "matchMode": "contains",
    "priority": 10,
    "enabled": true,
    "patterns": ["PPV", "PAY PER VIEW"]
  },
  {
    "kind": "flag",
    "value": "vip",
    "matchTarget": "channel_or_category",
    "matchMode": "contains",
    "priority": 10,
    "enabled": true,
    "patterns": ["VIP"]
  }
]
```
