# Contributing

Thanks for considering contributing to Moidify!

## Bugs & Features

Open a [GitHub issue](https://github.com/Moid-M/moidify/issues). Include:
- What you expected vs what happened
- Steps to reproduce
- Server logs if relevant (`journalctl -u moidify.service -n 50`)

## Pull Requests

Keep them focused — one feature or fix per PR. If adding a feature, open an issue first to discuss.

## Development setup

```bash
git clone https://github.com/Moid-M/moidify.git
cd moidify
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python3 server.py
```

## Code style

- Python: follow existing patterns in the codebase
- JavaScript: vanilla JS, no framework, no build step
- No comments unless the logic is genuinely non-obvious
- No external dependencies unless absolutely necessary
