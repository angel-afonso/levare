//! Pack score and part presentations around their actual ink, keeping musical curves intact.
use super::*;

const GAP: f32 = 6.;

fn overlaps(a: f32, b: f32, left: f32, right: f32) -> bool {
    a <= right && b >= left
}

fn curve_at(curve: &Curve, x: f32) -> f32 {
    let t = ((x - curve.from.x) / (curve.to.x - curve.from.x)).clamp(0., 1.);
    let smooth = t * t * (3. - 2. * t);
    curve.from.y + (curve.to.y - curve.from.y) * smooth + 3. * t * (1. - t) * curve.bend
}

fn local_top(layout: &Layout, rows: &[usize], left: f32, right: f32, metrics: &Metrics) -> f32 {
    let mut top = rows
        .iter()
        .map(|&i| layout.measures[i].origin.y)
        .fold(f32::INFINITY, f32::min);
    for &i in rows {
        let bar = &layout.measures[i];
        for glyph in &bar.content.glyphs {
            let (l, t, r, _) = metrics.bounds(&glyph.text, glyph.rotated);
            if overlaps(
                bar.origin.x + glyph.position.x + l,
                bar.origin.x + glyph.position.x + r,
                left,
                right,
            ) {
                top = top.min(bar.origin.y + glyph.position.y + t);
            }
        }
        for line in &bar.content.lines {
            if overlaps(
                bar.origin.x + line.from.x.min(line.to.x),
                bar.origin.x + line.from.x.max(line.to.x),
                left,
                right,
            ) {
                top = top.min(bar.origin.y + line.from.y.min(line.to.y) - line.thickness / 2.);
            }
        }
        for segment in bar.content.beams.iter().flat_map(|g| &g.segments) {
            let l = segment
                .corners
                .iter()
                .map(|p| p.x)
                .fold(f32::INFINITY, f32::min)
                + bar.origin.x;
            let r = segment
                .corners
                .iter()
                .map(|p| p.x)
                .fold(f32::NEG_INFINITY, f32::max)
                + bar.origin.x;
            if overlaps(l, r, left, right) {
                for p in &segment.corners {
                    top = top.min(bar.origin.y + p.y);
                }
            }
        }
    }
    for curve in layout.curves.iter().filter(|c| {
        c.bend < 0.
            && rows.iter().any(|&i| {
                layout.measures[i].origin.y == c.staff_origin
                    && layout.measures[i].part == c.connection.start.part
            })
    }) {
        if overlaps(curve.from.x, curve.to.x, left, right) {
            let a = left.max(curve.from.x);
            let b = right.min(curve.to.x);
            for i in 0..=16 {
                top = top.min(curve_at(curve, a + (b - a) * i as f32 / 16.) - 1.);
            }
        }
    }
    top
}

pub(super) fn compact(layout: &mut Layout, metrics: &Metrics) {
    for index in 0..layout.systems.len() {
        let system = layout.systems[index].clone();
        let rows: Vec<_> = layout
            .measures
            .iter()
            .enumerate()
            .filter(|(_, m)| (system.first_measure..=system.last_measure).contains(&m.index))
            .map(|(i, _)| i)
            .collect();
        // Each instrument/staff owns its text and curve clearance. A cello
        // indication must not inherit the violin's upper envelope.
        let mut staff_rows: Vec<Vec<usize>> = Vec::new();
        for &i in &rows {
            let bar = &layout.measures[i];
            if let Some(group) = staff_rows.iter_mut().find(|group| {
                let first = &layout.measures[group[0]];
                first.part == bar.part && first.staff == bar.staff
            }) {
                group.push(i);
            } else {
                staff_rows.push(vec![i]);
            }
        }
        for rows in staff_rows {
            let mut labels: Vec<_> = rows
                .iter()
                .flat_map(|&m| {
                    layout.measures[m]
                        .content
                        .techniques
                        .iter()
                        .enumerate()
                        .map(move |(i, t)| (t.lane(), m, i))
                })
                .collect();
            labels.sort_by_key(|&(division, m, i)| (division, m, i));
            let mut occupied: Vec<(f32, f32, f32)> = Vec::new();
            for (_, m, i) in labels {
                let bar = &layout.measures[m];
                let label = &bar.content.techniques[i];
                let left = bar.origin.x + label.position.x;
                let right = left + label.width();
                let mut y = local_top(layout, &rows, left - GAP, right + GAP, metrics) - GAP - 4.;
                while let Some(&(_, _, other_y)) = occupied.iter().find(|&&(l, r, other_y)| {
                    overlaps(l - GAP, r + GAP, left, right) && (y - other_y).abs() < 20. + GAP
                }) {
                    y = other_y - 20. - GAP;
                }
                let origin_y = bar.origin.y;
                let content = Arc::make_mut(&mut layout.measures[m].content);
                let label = &mut content.techniques[i];
                let shift = y - origin_y - label.position.y;
                let mark = label.mark;
                label.position.y += shift;
                for hit in content.marks.iter_mut().filter(|hit| hit.mark == mark) {
                    hit.center.y += shift;
                }
                occupied.push((left, right, y));
            }
            for &m in &rows {
                if layout.measures[m].content.techniques.is_empty() {
                    continue;
                }
                let content = Arc::make_mut(&mut layout.measures[m].content);
                content.ink_top = content
                    .techniques
                    .iter()
                    .map(|t| t.position.y - 16.)
                    .fold(expression::notation_bounds(content, metrics).0, f32::min);
                content.top = content.ink_top.min(-42.);
            }
        }
        // Tempo clears only the ink beneath its own horizontal footprint.
        for t in 0..layout.tempos.len() {
            if !(system.first_measure..=system.last_measure).contains(&layout.tempos[t].measure) {
                continue;
            }
            let left = layout.tempos[t].position.x;
            let right = left + layout.tempos[t].width;
            let mut top = local_top(layout, &rows, left - GAP, right + GAP, metrics);
            for &m in &rows {
                let bar = &layout.measures[m];
                for label in &bar.content.techniques {
                    let x = bar.origin.x + label.position.x;
                    if overlaps(x, x + label.width(), left - GAP, right + GAP) {
                        top = top.min(bar.origin.y + label.position.y - 16.);
                    }
                }
            }
            for label in &layout.labels {
                if (system.top..=system.bottom).contains(&label.position.y)
                    && overlaps(
                        label.position.x,
                        label.position.x + label.text.chars().count() as f32 * 7.,
                        left - GAP,
                        right + GAP,
                    )
                {
                    top = top.min(label.position.y - STAFF_LABEL_SIZE);
                }
            }
            // The tempo lane stays above an ending bracket beneath it.
            for ending in layout
                .endings
                .iter()
                .filter(|ending| (system.top..=system.bottom).contains(&ending.lines[0].from.y))
            {
                for line in &ending.lines {
                    if overlaps(
                        line.from.x.min(line.to.x),
                        line.from.x.max(line.to.x),
                        left - GAP,
                        right + GAP,
                    ) {
                        top = top.min(line.from.y.min(line.to.y) - line.thickness / 2.);
                    }
                }
            }
            // In the unbroken horizontal view, successive headings can
            // share an x range. Separate their ink vertically, not by
            // increasing the rhythmic width of either measure.
            for previous in layout.tempos[..t].iter().filter(|previous| {
                (system.first_measure..=system.last_measure).contains(&previous.measure)
                    && overlaps(
                        previous.position.x,
                        previous.position.x + previous.width,
                        left - GAP,
                        right + GAP,
                    )
            }) {
                top = top.min(previous.position.y - 30.);
            }
            layout.tempos[t].position.y = top - GAP - 6.;
        }
        for i in 0..layout.rehearsals.len() {
            let label = &layout.rehearsals[i];
            if !(system.first_measure..=system.last_measure).contains(&label.measure) {
                continue;
            }
            let left = label.frame[0].from.x;
            let right = label.frame[0].to.x;
            let mut top = local_top(layout, &rows, left - GAP, right + GAP, metrics);
            for &m in &rows {
                let bar = &layout.measures[m];
                for text in &bar.content.techniques {
                    let x = bar.origin.x + text.position.x;
                    if overlaps(x, x + text.width(), left - GAP, right + GAP) {
                        top = top.min(bar.origin.y + text.position.y - 16.);
                    }
                }
            }
            for ending in &layout.endings {
                if (system.top..=system.bottom).contains(&ending.lines[0].from.y) {
                    for line in &ending.lines {
                        if overlaps(
                            line.from.x.min(line.to.x),
                            line.from.x.max(line.to.x),
                            left - GAP,
                            right + GAP,
                        ) {
                            top = top.min(line.from.y.min(line.to.y) - line.thickness / 2.);
                        }
                    }
                }
            }
            let label = &mut layout.rehearsals[i];
            let shift = (top - GAP - 5. - 0.8 - label.position.y).min(0.);
            label.position.y += shift;
            for line in &mut label.frame {
                line.from.y += shift;
                line.to.y += shift;
            }
        }
        // Trim each system to its painted ink, rather than accumulated reserves.
        let (mut top, mut bottom) = (f32::INFINITY, f32::NEG_INFINITY);
        let mut include = |a: f32, b: f32| {
            top = top.min(a);
            bottom = bottom.max(b);
        };
        for &m in &rows {
            let bar = &layout.measures[m];
            include(bar.origin.y, bar.origin.y + 40.);
            for glyph in &bar.content.glyphs {
                let (_, a, _, b) = metrics.bounds(&glyph.text, glyph.rotated);
                include(
                    bar.origin.y + glyph.position.y + a,
                    bar.origin.y + glyph.position.y + b,
                );
            }
            for line in &bar.content.lines {
                include(
                    bar.origin.y + line.from.y.min(line.to.y) - line.thickness / 2.,
                    bar.origin.y + line.from.y.max(line.to.y) + line.thickness / 2.,
                );
            }
            for p in bar
                .content
                .beams
                .iter()
                .flat_map(|g| &g.segments)
                .flat_map(|s| &s.corners)
            {
                include(bar.origin.y + p.y, bar.origin.y + p.y);
            }
            for label in bar
                .content
                .techniques
                .iter()
                .chain(&bar.content.expressions)
            {
                include(
                    bar.origin.y + label.position.y - 16.,
                    bar.origin.y + label.position.y + 4.,
                );
            }
            for lyric in &bar.content.lyrics {
                include(
                    bar.origin.y + lyric.position.y - 14.,
                    bar.origin.y + lyric.position.y + 5.,
                );
            }
        }
        for curve in layout.curves.iter().filter(|c| {
            rows.iter().any(|&m| {
                layout.measures[m].origin.y == c.staff_origin
                    && layout.measures[m].part == c.connection.start.part
            })
        }) {
            for i in 0..=16 {
                let y = curve_at(
                    curve,
                    curve.from.x + (curve.to.x - curve.from.x) * i as f32 / 16.,
                );
                include(y - 1., y + 1.);
            }
        }
        for hairpin in layout.hairpins.iter().filter(|h| {
            rows.iter().any(|&m| {
                layout.measures[m].origin.y == h.staff_origin
                    && layout.measures[m].part == h.connection.start.part
            })
        }) {
            for line in [&hairpin.upper, &hairpin.lower] {
                include(
                    line.from.y.min(line.to.y) - line.thickness / 2.,
                    line.from.y.max(line.to.y) + line.thickness / 2.,
                );
            }
        }
        for line in layout.line_marks.iter().filter(|l| {
            rows.iter().any(|&m| {
                layout.measures[m].origin.y == l.staff_origin
                    && layout.measures[m].part == l.connection.start.part
            })
        }) {
            let (_, top, _, bottom) = line.ink();
            include(top, bottom);
        }
        for arpeggio in layout.piano_arpeggios.iter().filter(|a| {
            (system.first_measure..=system.last_measure).contains(&a.other_hand.measure)
        }) {
            let (_, a, _, b) = metrics.bounds(&arpeggio.glyph.text, arpeggio.glyph.rotated);
            include(arpeggio.glyph.position.y + a, arpeggio.glyph.position.y + b);
        }
        for label in layout
            .labels
            .iter()
            .filter(|l| (system.top..=system.bottom).contains(&l.position.y))
        {
            include(label.position.y - STAFF_LABEL_SIZE, label.position.y + 4.);
        }
        for tempo in layout
            .tempos
            .iter()
            .filter(|t| (system.first_measure..=system.last_measure).contains(&t.measure))
        {
            include(tempo.position.y - 30., tempo.position.y + 6.);
        }
        for ending in layout
            .endings
            .iter()
            .filter(|ending| (system.top..=system.bottom).contains(&ending.lines[0].from.y))
        {
            for line in &ending.lines {
                include(
                    line.from.y.min(line.to.y) - line.thickness / 2.,
                    line.from.y.max(line.to.y) + line.thickness / 2.,
                );
            }
            if !ending.label.is_empty() {
                include(ending.label_position.y - 14., ending.label_position.y + 4.);
            }
        }
        for label in layout
            .rehearsals
            .iter()
            .filter(|label| (system.first_measure..=system.last_measure).contains(&label.measure))
        {
            for line in &label.frame {
                include(
                    line.from.y.min(line.to.y) - 0.8,
                    line.from.y.max(line.to.y) + 0.8,
                );
            }
        }
        for label in layout
            .navigation
            .iter()
            .filter(|label| (system.first_measure..=system.last_measure).contains(&label.measure))
        {
            include(label.position.y - label.size, label.position.y + 4.);
        }
        layout.systems[index].top = top;
        layout.systems[index].bottom = bottom;
    }
    layout.height = layout.systems.last().unwrap().bottom + 24.;
}
