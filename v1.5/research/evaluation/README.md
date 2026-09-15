# IDR V1.5 offline evaluation

Run `python -m evaluation.evaluate records.jsonl` from `v1.5/research`.

The evaluator consumes `EvaluationRecordV1` JSONL and reports intent correction,
decision override, re-clarification, outcome success, and host-model invocation
rates. It does not require or retain raw conversations.
