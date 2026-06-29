# jewell-labs

Installable [Hermes](https://github.com/) **skills**, pulled directly with the
`hermes` CLI. Each skill is a directory containing a `SKILL.md` (plus any helper
scripts under `scripts/`).

## Install a skill

```sh
hermes skills install tyler-jewell/jewell-labs/skills/<name>
```

## Or tap the whole repo as a skill source

```sh
hermes skills tap add tyler-jewell/jewell-labs
hermes skills search <query>      # then: hermes skills install <name>
```

Skills stay current via `hermes skills check` → `hermes skills update` (no
`.git` checkout needed — they refetch by identifier).

## Skills

| Skill | What it does |
|-------|--------------|
| [`extending-hermes-dashboard`](skills/extending-hermes-dashboard) | Step-by-step guide + scaffold for building, testing, and installing a Hermes dashboard plugin. |

## Plugins

Hermes **plugins** live in their own repos (one per plugin, plugin at the repo
root) so native `hermes plugins update` (`git pull`) keeps them current:

- **local-llm-server** — `hermes plugins install tyler-jewell/hermes-plugin-local-llm-server --enable`
- **benchmark-results** — `hermes plugins install tyler-jewell/hermes-plugin-benchmark-results --enable`

## License

MIT
