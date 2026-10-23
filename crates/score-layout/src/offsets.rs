//! Move written marks after automatic engraving has finished.
use super::*;

impl Layout {
    pub fn translate_bar_mark(&mut self, reference: BarMarkRef, dx: f32, dy: f32) {
        if dx == 0. && dy == 0. {
            return;
        }
        match reference.mark {
            BarMark::Tempo => {
                for label in self
                    .tempos
                    .iter_mut()
                    .filter(|label| label.measure == reference.measure)
                {
                    label.position.x += dx;
                    label.position.y += dy;
                }
            }
            BarMark::Rehearsal => {
                for label in self
                    .rehearsals
                    .iter_mut()
                    .filter(|label| label.measure == reference.measure)
                {
                    label.position.x += dx;
                    label.position.y += dy;
                    for line in &mut label.frame {
                        for point in [&mut line.from, &mut line.to] {
                            point.x += dx;
                            point.y += dy;
                        }
                    }
                }
            }
            BarMark::Ending => {
                for bracket in self
                    .endings
                    .iter_mut()
                    .filter(|bracket| bracket.ending_start == reference.measure)
                {
                    bracket.label_position.x += dx;
                    bracket.label_position.y += dy;
                    for line in &mut bracket.lines {
                        for point in [&mut line.from, &mut line.to] {
                            point.x += dx;
                            point.y += dy;
                        }
                    }
                }
            }
            _ => {
                for label in self.navigation.iter_mut().filter(|label| {
                    label.measure == reference.measure && label.mark == reference.mark
                }) {
                    label.position.x += dx;
                    label.position.y += dy;
                }
            }
        }
    }

    /// Shift one mark's ink and clickable area together, in layout units.
    pub fn translate_mark(&mut self, mark: MarkRef, dx: f32, dy: f32) {
        if dx == 0. && dy == 0. {
            return;
        }
        if let MarkRef::Connection(connection) = mark {
            for curve in self
                .curves
                .iter_mut()
                .filter(|curve| curve.connection == connection)
            {
                curve.from.x += dx;
                curve.from.y += dy;
                curve.to.x += dx;
                curve.to.y += dy;
            }
            for hairpin in self
                .hairpins
                .iter_mut()
                .filter(|hairpin| hairpin.connection == connection)
            {
                for point in [
                    &mut hairpin.upper.from,
                    &mut hairpin.upper.to,
                    &mut hairpin.lower.from,
                    &mut hairpin.lower.to,
                ] {
                    point.x += dx;
                    point.y += dy;
                }
            }
            return;
        }
        for placed in &mut self.measures {
            let indices: Vec<_> = placed
                .content
                .marks
                .iter()
                .enumerate()
                .filter_map(|(index, box_)| (box_.mark == mark).then_some(index))
                .collect();
            if indices.is_empty() {
                continue;
            }
            let content = Arc::make_mut(&mut placed.content);
            let mut moved_glyphs = HashSet::new();
            for index in indices {
                let box_ = &mut content.marks[index];
                box_.center.x += dx;
                box_.center.y += dy;
                content.ink_top = content.ink_top.min(box_.center.y - box_.half.1 - 4.);
                content.ink_bottom = content.ink_bottom.max(box_.center.y + box_.half.1 + 4.);
                if let Some(glyph) = box_.glyph
                    && moved_glyphs.insert(glyph)
                {
                    content.glyphs[glyph].position.x += dx;
                    content.glyphs[glyph].position.y += dy;
                }
            }
            for label in content
                .techniques
                .iter_mut()
                .chain(&mut content.expressions)
                .filter(|label| label.mark == mark)
            {
                label.position.x += dx;
                label.position.y += dy;
            }
        }
    }
}

pub(super) fn apply_bar_marks(score: &Score, layout: &mut Layout) {
    for (&measure, offsets) in score.bar_offsets() {
        for mark in BarMark::ALL {
            if let Some(offset) = offsets.get(mark) {
                layout.translate_bar_mark(
                    BarMarkRef { measure, mark },
                    f32::from(offset.dx),
                    f32::from(offset.dy),
                );
            }
        }
    }
}

/// Engraving decides placement first; written offsets move marks last.
pub(super) fn apply(score: &Score, layout: &mut Layout) {
    let marks: HashSet<_> = layout
        .measures
        .iter()
        .flat_map(|placed| placed.content.marks.iter().map(|box_| box_.mark))
        .collect();
    for mark in marks {
        let offset = score.mark_offset(mark);
        if !offset.is_zero() {
            layout.translate_mark(mark, f32::from(offset.dx), f32::from(offset.dy));
        }
    }
    for entry in score.connection_offsets() {
        layout.translate_mark(
            MarkRef::Connection(entry.connection),
            f32::from(entry.offset.dx),
            f32::from(entry.offset.dy),
        );
    }
}
