# Embeddable Runtime contracts V1

These JSON Schema 2020-12 documents are the language-neutral boundary for the
V1.5 reference implementation. Schema version `1.0` is intentionally separate
from the frozen V1.3 protocol and runtime contracts.

Raw user input may be present in a `ResolveRequestV1`, but no contract requires
retaining a raw conversation. Offline improvement uses `EvaluationRecordV1`.

`HostModelResultV1` is structured and validated by IDR; a provider response
containing only free-form `content` is not a valid continuation.

