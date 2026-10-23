//! Lane layout of the commit graph (what `git log --graph` draws, but as data a GUI can paint).
//!
//! Commits come newest first, each with its parents. Every branch of history occupies a *lane*: a column that
//! runs down until the commit it is waiting for shows up. For one commit the layout says in which lane it sits,
//! which lanes end in it (children), which lanes pass by, and into which lanes its parents continue.

use crate::git::Commit;

/// A lane and the colour index of the branch line on it.
pub type Lane = (usize, usize);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphRow {
    /// Lane of the commit itself and its colour index.
    pub lane: usize,
    pub color: usize,
    /// Lanes that come from above and end in this commit (one per child).
    pub incoming: Vec<Lane>,
    /// Lanes that run straight through this row.
    pub through: Vec<Lane>,
    /// Lanes the parents continue in (the first one is usually the commit's own lane).
    pub outgoing: Vec<Lane>,
    /// Number of lanes this row touches.
    pub width: usize,
}

/// Lay out `commits` (newest first). The result has one row per commit.
pub fn layout(commits: &[Commit]) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<(String, usize)>> = Vec::new();
    let mut next_color = 0usize;
    let mut rows = Vec::with_capacity(commits.len());
    for c in commits {
        let converged: Vec<usize> = lanes.iter().enumerate().filter(|(_, l)| l.as_ref().is_some_and(|(h, _)| *h == c.hash)).map(|(i, _)| i).collect();
        let (lane, color) = match converged.first() {
            Some(&i) => (i, lanes[i].as_ref().map_or(0, |l| l.1)),
            None => {
                let i = free_slot(&mut lanes);
                next_color += 1;
                (i, next_color - 1)
            }
        };
        let incoming: Vec<Lane> = converged.iter().map(|&i| (i, lanes[i].as_ref().map_or(0, |l| l.1))).collect();
        let through: Vec<Lane> = lanes.iter().enumerate().filter(|(i, _)| !converged.contains(i)).filter_map(|(i, l)| l.as_ref().map(|l| (i, l.1))).collect();
        for &i in &converged {
            lanes[i] = None;
        }
        let mut outgoing: Vec<Lane> = Vec::new();
        for (k, p) in c.parents.iter().enumerate() {
            if let Some(j) = lanes.iter().position(|l| l.as_ref().is_some_and(|(h, _)| h == p)) {
                outgoing.push((j, lanes[j].as_ref().map_or(0, |l| l.1)));
                continue;
            }
            // the first parent continues the commit's own lane when that lane is free (or new)
            let (slot, col) = if k == 0 && lanes.get(lane).is_none_or(|l| l.is_none()) {
                (lane, color)
            } else {
                next_color += 1;
                (free_slot(&mut lanes), next_color - 1)
            };
            if slot >= lanes.len() {
                lanes.resize(slot + 1, None);
            }
            lanes[slot] = Some((p.clone(), col));
            outgoing.push((slot, col));
        }
        while lanes.last().is_some_and(|l| l.is_none()) {
            lanes.pop();
        }
        let width = [lane + 1, incoming.iter().map(|l| l.0 + 1).max().unwrap_or(0), through.iter().map(|l| l.0 + 1).max().unwrap_or(0), outgoing.iter().map(|l| l.0 + 1).max().unwrap_or(0)].into_iter().max().unwrap_or(1);
        rows.push(GraphRow { lane, color, incoming, through, outgoing, width });
    }
    rows
}

fn free_slot(lanes: &mut Vec<Option<(String, usize)>>) -> usize {
    match lanes.iter().position(|l| l.is_none()) {
        Some(i) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(h: &str, parents: &[&str]) -> Commit {
        Commit { hash: h.into(), short: h.into(), author: String::new(), when: String::new(), subject: String::new(), parents: parents.iter().map(|p| p.to_string()).collect(), refs: vec![] }
    }

    #[test]
    fn linear_history_stays_in_one_lane() {
        let rows = layout(&[c("c", &["b"]), c("b", &["a"]), c("a", &[])]);
        assert!(rows.iter().all(|r| r.lane == 0 && r.width == 1));
        assert_eq!(rows[0].outgoing, vec![(0, 0)]);
        assert_eq!(rows[1].incoming, vec![(0, 0)]);
        assert!(rows[2].outgoing.is_empty());
    }

    #[test]
    fn branch_and_merge_use_two_lanes() {
        // m merges f (feature) into main; both come from base b
        let rows = layout(&[c("m", &["a", "f"]), c("f", &["b"]), c("a", &["b"]), c("b", &[])]);
        assert_eq!(rows[0].lane, 0);
        assert_eq!(rows[0].outgoing.len(), 2);
        let feature_lane = rows[0].outgoing[1].0;
        assert_eq!(feature_lane, 1);
        // f sits in the feature lane, a in the main lane, and a lane runs past each of them
        assert_eq!(rows[1].lane, 1);
        assert_eq!(rows[1].through, vec![(0, 0)]);
        assert_eq!(rows[2].lane, 0);
        assert_eq!(rows[2].through.len(), 1);
        // the main line merges into the feature lane (a's parent b is already waited for there)
        assert_eq!(rows[2].outgoing, vec![(1, rows[1].color)]);
        assert_eq!(rows[3].lane, 1);
        assert_eq!(rows[3].incoming, vec![(1, rows[1].color)]);
    }

    #[test]
    fn a_second_tip_joins_the_lane_waiting_for_the_same_parent() {
        let rows = layout(&[c("x", &["r"]), c("y", &["r"]), c("r", &[])]);
        assert_eq!(rows[0].lane, 0);
        assert_eq!(rows[1].lane, 1);
        // y has no child of its own, so it gets a new lane; its line joins the lane that already waits for r
        assert_eq!(rows[1].outgoing, vec![(0, rows[0].color)]);
        assert_eq!(rows[2].lane, 0);
        assert_eq!(rows[2].incoming, vec![(0, rows[0].color)]);
    }

    #[test]
    fn a_missing_parent_leaves_a_dangling_lane() {
        let rows = layout(&[c("b", &["gone"]), c("a", &[])]);
        assert_eq!(rows[0].outgoing, vec![(0, 0)]);
        assert_eq!(rows[1].lane, 1);
        assert_eq!(rows[1].through, vec![(0, 0)]);
    }
}
