# IDR Evaluation

Python is used here for offline research and evaluation only.

This package may score:

- intent hypothesis preservation;
- guard-output agreement against golden fixtures;
- confirmation-before-action behavior;
- uncertainty and evidence disclosure coverage.

It does not grant runtime authority, mutate contracts, promote Human Model
assertions, or execute actions.

Run the standard-library test suite:

```bash
PYTHONPATH=src python3 -m unittest discover -s tests -v
```
