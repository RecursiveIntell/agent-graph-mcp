from __future__ import annotations

import re
from typing import Any

_STOP = {"the", "a", "an", "and", "or", "to", "of", "in", "on", "for", "this", "that", "is", "are", "be", "with", "before", "after", "without"}


def _tokens(text: str) -> set[str]:
    tokens = {t for t in re.findall(r"[a-z0-9][a-z0-9_-]*", text.lower()) if t not in _STOP and len(t) > 2}
    expanded = set(tokens)
    for token in tokens:
        if token.endswith("ance"):
            expanded.add(token[:-4] + "e")
        if token.endswith("ing"):
            expanded.add(token[:-3])
        if token.endswith("ed"):
            expanded.add(token[:-2])
        if token.endswith("s"):
            expanded.add(token[:-1])
    return expanded


def _criterion_hit(criterion: str, text: str) -> bool:
    haystack = text.lower()
    phrase = re.sub(r"[^a-z0-9 ]", " ", criterion.lower())
    if phrase.strip() in haystack:
        return True
    if "quiesce" in phrase and "copy" in phrase and "copy" in haystack and ("unsafe" in haystack or "live-write" in haystack or "quiescen" in haystack):
        return True
    if "verif" in phrase and "verif" in haystack:
        if "deletion" not in phrase or "delet" in haystack:
            return True
    required = _tokens(criterion)
    present = _tokens(text)
    overlap = sum(1 for item in required if any(item[:5] == candidate[:5] for candidate in present))
    return bool(required) and overlap / len(required) >= 0.40


def _denial_hit(criterion: str, text: str) -> bool:
    haystack = text.lower()
    criterion_lower = criterion.lower()
    if "approves deletion without verification" in criterion_lower:
        return ("approve" in haystack or "safe" in haystack) and "verif" not in haystack
    if "treating inference as verified fact" in criterion_lower or "treats inference as verified" in criterion_lower:
        return ("is true" in haystack or "verified" in haystack or "definitely" in haystack) and "evidence" not in haystack and "inference" not in haystack
    if "missing-success-run evidence" in criterion_lower:
        has_explicit_gap = ("missing" in haystack and "evidence" in haystack) or (
            "no successful" in haystack and any(marker in haystack for marker in ("record", "run", "rebuild"))
        ) or ("unverified" in haystack and any(marker in haystack for marker in ("success", "run", "rebuild")))
        return not has_explicit_gap
    if criterion_lower.startswith("missing "):
        target = criterion[8:].strip()
        if "live-write inconsistency" in target.lower():
            has_live_writers = "writ" in haystack and any(
                marker in haystack for marker in ("live", "writable", "continues", "active", "remain")
            )
            has_consistency_hazard = any(marker in haystack for marker in ("inconsist", "incoherent", "quiescen", "point-in-time"))
            return not (has_live_writers and has_consistency_hazard)
        return not _criterion_hit(target, text)
    if "both claims compatible" in criterion_lower:
        declares_compatibility = (
            "both claims" in haystack and "compatible" in haystack
        ) or "claims are compatible" in haystack
        scoped_compatibility = any(
            marker in haystack
            for marker in ("only if", "conditional", "provided that", "depends on", "scope")
        )
        return declares_compatibility and not scoped_compatibility
    if "speculative fix" in criterion_lower or "no reproduction command" in criterion_lower:
        return not any(marker in haystack for marker in ("diagnostic", "hypothes", "discriminating test", "command"))
    if "releasing while the claim is unsupported" in criterion_lower:
        return "block" not in haystack or "unsupported" not in haystack
    if "counting warnings as release blockers" in criterion_lower:
        return not any(marker in haystack for marker in ("separate", "pre-existing", "warnings"))
    if "recommending a swap without a pilot" in criterion_lower:
        return not any(marker in haystack for marker in ("pilot", "experiment"))
    return _criterion_hit(criterion, text)


def adjudicate_text(task: dict[str, Any], text: str) -> dict[str, Any]:
    acceptance = task.get("acceptance_criteria", [])
    denials = task.get("denial_criteria", [])
    matched = [criterion for criterion in acceptance if _criterion_hit(criterion, text)]
    denial_failures = [criterion for criterion in denials if _denial_hit(criterion, text)]
    score = round(len(matched) / len(acceptance), 6) if acceptance else 0.0
    if denial_failures:
        score = 0.0
    return {
        "score": score,
        "acceptance_total": len(acceptance),
        "acceptance_matched": matched,
        "acceptance_missing": [c for c in acceptance if c not in matched],
        "denial_failures": denial_failures,
        "judge": "deterministic_criterion_match_v1",
    }
