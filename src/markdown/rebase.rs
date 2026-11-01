//! Map authored offsets after renderer replacements without revisiting every
//! stored position for each replacement.

use std::ops::Range;

pub(crate) struct Rebase {
    edits: Vec<(Range<usize>, isize)>,
}

impl Rebase {
    /// Replacements arrive in reverse source order, as they are applied.
    pub(crate) fn new(replacements: &[(Range<usize>, usize)]) -> Result<Self, Range<usize>> {
        let mut edits = Vec::with_capacity(replacements.len());
        let mut delta = 0isize;
        for (span, length) in replacements.iter().rev() {
            let original = isize::try_from(span.end - span.start).map_err(|_| span.clone())?;
            let replacement = isize::try_from(*length).map_err(|_| span.clone())?;
            delta = delta
                .checked_add(replacement - original)
                .ok_or_else(|| span.clone())?;
            edits.push((span.clone(), delta));
        }
        Ok(Self { edits })
    }

    fn boundary(&self, position: usize) -> Result<usize, Range<usize>> {
        let index = self.edits.partition_point(|(span, _)| span.end <= position);
        if let Some((span, _)) = self.edits.get(index)
            && span.start < position
            && position < span.end
        {
            return Err(span.clone());
        }
        let delta = index.checked_sub(1).map_or(0, |i| self.edits[i].1);
        position.checked_add_signed(delta).ok_or_else(|| {
            self.edits
                .get(index.saturating_sub(1))
                .map_or(position..position, |edit| edit.0.clone())
        })
    }

    pub(crate) fn point(&self, position: usize) -> Result<usize, Range<usize>> {
        let index = self.edits.partition_point(|(span, _)| span.end <= position);
        if let Some((span, _)) = self.edits.get(index)
            && span.contains(&position)
        {
            return Err(span.clone());
        }
        self.boundary(position)
    }

    pub(crate) fn range(
        &self,
        span: &Range<usize>,
        allow_contained: bool,
    ) -> Result<Range<usize>, Range<usize>> {
        if !allow_contained {
            let index = self
                .edits
                .partition_point(|(edit, _)| edit.end <= span.start);
            if let Some((edit, _)) = self.edits.get(index)
                && edit.start < span.end
            {
                return Err(span.clone());
            }
        }
        Ok(self.boundary(span.start)?..self.boundary(span.end)?)
    }
}

#[cfg(test)]
mod tests {
    use super::Rebase;

    #[test]
    fn maps_multiple_edits_and_detects_overlaps() {
        let rebase = Rebase::new(&[(20..25, 2), (4..7, 8)]).unwrap();
        assert_eq!(rebase.point(3).unwrap(), 3);
        assert_eq!(rebase.point(7).unwrap(), 12);
        assert_eq!(rebase.point(25).unwrap(), 27);
        assert_eq!(rebase.range(&(8..12), false).unwrap(), 13..17);
        assert_eq!(rebase.range(&(2..25), true).unwrap(), 2..27);
        assert!(rebase.range(&(2..25), false).is_err());
        assert!(rebase.point(4).is_err());
        assert!(rebase.range(&(5..9), true).is_err());
    }

    #[test]
    fn maps_adjacent_edits_and_ranges_at_their_boundaries() {
        let rebase = Rebase::new(&[(7..10, 1), (4..7, 5)]).unwrap();
        assert_eq!(rebase.range(&(1..4), false).unwrap(), 1..4);
        assert_eq!(rebase.range(&(10..12), false).unwrap(), 10..12);
        assert_eq!(rebase.range(&(4..10), true).unwrap(), 4..10);
        assert_eq!(rebase.range(&(2..12), true).unwrap(), 2..12);
        assert!(rebase.range(&(4..7), false).is_err());
        assert!(rebase.point(7).is_err());
    }

    #[test]
    fn maps_a_component_body_spanning_several_replacements() {
        let rebase = Rebase::new(&[(20..25, 2), (4..7, 8)]).unwrap();
        assert_eq!(rebase.range(&(2..30), true).unwrap(), 2..32);
        assert_eq!(rebase.range(&(2..20), true).unwrap(), 2..25);
        assert!(rebase.range(&(5..30), true).is_err());
        assert!(rebase.range(&(2..22), true).is_err());
    }
}
