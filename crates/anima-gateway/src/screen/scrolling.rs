//! Finding a scroll in an area; the answer is only a hint, as the screen compares the pixels after it.

use std::hash::{DefaultHasher, Hasher as _};

/// Areas with fewer rows are cheap to send as they are.
const FEWEST_ROWS: usize = 48;
const MATCH_SHARE: (usize, usize) = (3, 4);
const PROBES: usize = 3;

/// The row at `from` and the `rows - 1` below it are now at `to`, counted from the area's first row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Scroll {
    pub(super) from: usize,
    pub(super) to: usize,
    pub(super) rows: usize,
}

pub(super) fn row_marks<'a>(rows: impl Iterator<Item = &'a [u8]>) -> Vec<u64> {
    rows.map(|row| {
        let mut hasher = DefaultHasher::new();
        hasher.write(row);
        hasher.finish()
    })
    .collect()
}

pub(super) fn scroll(old: &[u64], new: &[u64]) -> Option<Scroll> {
    let rows = new.len();
    if rows < FEWEST_ROWS || old.len() != rows {
        return None;
    }
    let unchanged = old.iter().zip(new).filter(|(old, new)| old == new).count();
    if unchanged * 2 > rows {
        return None;
    }
    (1..=PROBES)
        .map(|probe| probe * rows / (PROBES + 1))
        .flat_map(|at| {
            let wanted = new[at];
            (0..rows)
                .filter(move |&row| row != at && old[row] == wanted)
                .map(move |row| (row, at))
        })
        .take(PROBES * 4)
        .map(|(was, is)| moved_together(was, is, rows))
        .find(|scroll| matches(old, new, *scroll))
}

const fn moved_together(was: usize, is: usize, rows: usize) -> Scroll {
    if was > is {
        let by = was - is;
        Scroll {
            from: by,
            to: 0,
            rows: rows - by,
        }
    } else {
        let by = is - was;
        Scroll {
            from: 0,
            to: by,
            rows: rows - by,
        }
    }
}

fn matches(old: &[u64], new: &[u64], scroll: Scroll) -> bool {
    let was = &old[scroll.from..scroll.from + scroll.rows];
    let is = &new[scroll.to..scroll.to + scroll.rows];
    let equal = was.iter().zip(is).filter(|(was, is)| was == is).count();
    equal * MATCH_SHARE.1 >= scroll.rows * MATCH_SHARE.0
}
