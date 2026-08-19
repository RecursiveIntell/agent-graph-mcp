//! Automatic retention GC policy evaluation (B7 / Sprint D).
//!
//! Pure decision logic: given candidate graph rows and a policy, decide which
//! non-destructive retention transitions to propose. GC **never** proposes a
//! destructive transition (nothing past `expired_pending_review`); purging
//! remains operator-gated. Pinned and legal-held graphs are always skipped.
//!
//! The daemon applies the proposed transitions through the standard
//! receipt-bearing store path (`store::gc_run_policy`).

/// Persisted GC policy (row id=1 in `gc_policy`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcPolicy {
    pub enabled: bool,
    pub idle_archive_days: i64,
    pub review_expire_days: i64,
    pub min_executions: i64,
    pub storage_flag_mb: i64,
    pub last_run: Option<String>,
}

impl Default for GcPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            idle_archive_days: 30,
            review_expire_days: 60,
            min_executions: 5,
            storage_flag_mb: 10,
            last_run: None,
        }
    }
}

/// One graph's retention-relevant facts for evaluation.
#[derive(Debug, Clone)]
pub struct GcCandidateRow {
    pub graph: String,
    pub state: String,
    pub last_execution_at: Option<String>,
    pub execution_count: i64,
    pub pinned: bool,
    pub legal_held: bool,
    pub storage_bytes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GcAction {
    /// Propose `active -> archived` (idle past `idle_archive_days`).
    Archive { graph: String },
    /// Propose `archived -> expired_pending_review` (idle past
    /// `review_expire_days`, or low-value fast-track).
    Expire { graph: String },
    /// Flag for operator review only (storage above `storage_flag_mb`).
    FlagStorage { graph: String },
}

/// Evaluate the policy against candidate rows. Pure — no I/O, no side effects.
pub fn gc_decisions(rows: &[GcCandidateRow], policy: &GcPolicy, now: &str) -> Vec<GcAction> {
    if !policy.enabled {
        return Vec::new();
    }
    let mut actions = Vec::new();
    for row in rows {
        if row.pinned || row.legal_held {
            continue;
        }
        let idle_days = idle_days_before(row.last_execution_at.as_deref(), now);
        match row.state.as_str() {
            "active" | "active_phantom_contaminated" => {
                if idle_days >= policy.idle_archive_days {
                    actions.push(GcAction::Archive {
                        graph: row.graph.clone(),
                    });
                }
            }
            "archived" => {
                let idle_long = idle_days >= policy.review_expire_days;
                let low_value = row.execution_count < policy.min_executions
                    && idle_days >= policy.idle_archive_days;
                if idle_long || low_value {
                    actions.push(GcAction::Expire {
                        graph: row.graph.clone(),
                    });
                }
            }
            _ => {}
        }
        if row.storage_bytes >= policy.storage_flag_mb * 1024 * 1024 {
            actions.push(GcAction::FlagStorage {
                graph: row.graph.clone(),
            });
        }
    }
    actions
}

/// Whole days between `last_execution_at` (SQLite `YYYY-MM-DD HH:MM:SS` or
/// ISO) and `now` (same formats). No timestamp at all counts as fully idle.
pub fn idle_days_before(last_execution_at: Option<&str>, now: &str) -> i64 {
    let Some(last) = last_execution_at else {
        return i64::MAX;
    };
    let parse = |s: &str| -> Option<i64> {
        // Accept "YYYY-MM-DD HH:MM:SS" and "YYYY-MM-DDTHH:MM:SS(.f)(Z|+..)".
        let s = s.trim();
        let (date, time) = match s.find([' ', 'T']) {
            Some(i) => (&s[..i], &s[i + 1..]),
            None => (s, ""),
        };
        let mut parts = date.split('-');
        let y: i64 = parts.next()?.parse().ok()?;
        let m: i64 = parts.next()?.parse().ok()?;
        let d: i64 = parts.next()?.parse().ok()?;
        let t: i64 = time
            .split([':', '+', 'Z', '.'])
            .next()
            .unwrap_or("")
            .parse()
            .ok()
            .unwrap_or(0);
        Some(y * 372 + m * 31 + d + t / 86400)
    };
    let (Some(a), Some(b)) = (parse(last), parse(now)) else {
        return i64::MAX;
    };
    (b - a).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(graph: &str, state: &str, last: Option<&str>, execs: i64) -> GcCandidateRow {
        GcCandidateRow {
            graph: graph.into(),
            state: state.into(),
            last_execution_at: last.map(str::to_owned),
            execution_count: execs,
            pinned: false,
            legal_held: false,
            storage_bytes: 0,
        }
    }

    const NOW: &str = "2026-08-19 00:00:00";

    #[test]
    fn idle_days_parsing() {
        assert_eq!(idle_days_before(Some("2026-07-19 00:00:00"), NOW), 31);
        assert_eq!(idle_days_before(Some("2026-08-19T00:00:00Z"), NOW), 0);
        assert_eq!(idle_days_before(None, NOW), i64::MAX);
        assert_eq!(idle_days_before(Some("2026-08-20 00:00:00"), NOW), 0);
    }

    #[test]
    fn archives_idle_active_graphs() {
        let policy = GcPolicy::default();
        let rows = vec![
            row("idle", "active", Some("2026-06-01 00:00:00"), 3),
            row("fresh", "active", Some("2026-08-18 00:00:00"), 3),
            row("nola", "active", None, 0),
        ];
        let actions = gc_decisions(&rows, &policy, NOW);
        assert!(actions.contains(&GcAction::Archive {
            graph: "idle".into()
        }));
        assert!(actions.contains(&GcAction::Archive {
            graph: "nola".into()
        }));
        assert!(!actions.contains(&GcAction::Archive {
            graph: "fresh".into()
        }));
    }

    #[test]
    fn expires_archived_by_age_and_low_value() {
        let policy = GcPolicy::default();
        let rows = vec![
            row("old", "archived", Some("2026-05-01 00:00:00"), 10), // >60d idle
            row("few", "archived", Some("2026-07-01 00:00:00"), 2),  // <5 execs + >30d
            row("young", "archived", Some("2026-08-10 00:00:00"), 10), // neither
        ];
        let actions = gc_decisions(&rows, &policy, NOW);
        assert!(actions.contains(&GcAction::Expire {
            graph: "old".into()
        }));
        assert!(actions.contains(&GcAction::Expire {
            graph: "few".into()
        }));
        assert!(!actions.contains(&GcAction::Expire {
            graph: "young".into()
        }));
    }

    #[test]
    fn pinned_and_held_skipped() {
        let policy = GcPolicy::default();
        let mut pinned = row("pin", "active", Some("2026-06-01 00:00:00"), 0);
        pinned.pinned = true;
        let mut held = row("hold", "active", Some("2026-06-01 00:00:00"), 0);
        held.legal_held = true;
        let actions = gc_decisions(&[pinned, held], &policy, NOW);
        assert!(actions.is_empty());
    }

    #[test]
    fn storage_flag_independent_of_state() {
        let policy = GcPolicy::default();
        let mut big = row("big", "active", Some("2026-08-18 00:00:00"), 0);
        big.storage_bytes = 11 * 1024 * 1024;
        let actions = gc_decisions(&[big], &policy, NOW);
        assert!(actions.contains(&GcAction::FlagStorage {
            graph: "big".into()
        }));
    }

    #[test]
    fn disabled_policy_returns_nothing() {
        let policy = GcPolicy {
            enabled: false,
            ..Default::default()
        };
        let rows = vec![row("idle", "active", Some("2026-06-01 00:00:00"), 0)];
        assert!(gc_decisions(&rows, &policy, NOW).is_empty());
    }
}
