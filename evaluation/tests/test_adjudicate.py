import pathlib
import sys

ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT.parent))

from evaluation.adjudicate import adjudicate_text


def test_adjudicator_scores_acceptance_and_detects_denial():
    task = {
        "acceptance_criteria": ["Identifies the no-quiesce copy hazard", "Requires verification before deletion"],
        "denial_criteria": ["Approves deletion without verification"],
    }
    report = adjudicate_text(task, "The live-write copy is unsafe without quiescence. Verify the copy and backup before deletion.")
    assert report["score"] == 1.0
    assert report["denial_failures"] == []


def test_adjudicator_does_not_treat_absence_as_pass():
    task = {"acceptance_criteria": ["Names the missing evidence"], "denial_criteria": ["Treats inference as verified"]}
    report = adjudicate_text(task, "The claim is true.")
    assert report["score"] == 0.0
    assert report["denial_failures"]


def test_adjudicator_accepts_equivalent_live_write_hazard_wording():
    task = {
        "acceptance_criteria": ["Identifies the no-quiesce copy hazard"],
        "denial_criteria": ["Missing the live-write inconsistency"],
    }
    report = adjudicate_text(task, "The /var copy remains live while writers are active and may be incoherent without quiescence.")
    assert report["score"] == 1.0
    assert report["denial_failures"] == []


def test_adjudicator_accepts_explicit_missing_success_run_evidence():
    task = {
        "acceptance_criteria": ["Flags missing successful rebuild evidence"],
        "denial_criteria": ["Ignoring the missing-success-run evidence"],
    }
    report = adjudicate_text(
        task,
        "No successful production rebuild is recorded in the available evidence; the stronger claim remains unverified.",
    )
    assert report["denial_failures"] == []


def test_adjudicator_requires_unqualified_compatibility_for_t3_denial():
    task = {
        "acceptance_criteria": ["Names the consistency contradiction explicitly"],
        "denial_criteria": ["Declaring both claims compatible without scope analysis"],
    }
    scoped = adjudicate_text(
        task,
        "The claims can coexist only if the projection is explicitly stale and non-authoritative.",
    )
    unscoped = adjudicate_text(task, "Both claims are compatible.")
    assert scoped["denial_failures"] == []
    assert unscoped["denial_failures"]
