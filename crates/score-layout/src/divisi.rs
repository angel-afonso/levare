//! Whole-bar staff presentation with exact divisi/unison timing labels.
use super::*;
use score_core::{Division, MarkRef, Part};

pub(super) fn regions(part: &Part, staff: StaffId, bar: usize, capacity: u16) -> Vec<VoiceRegion> {
    let voices: &[Voice] =
        if !part.instrument().is_grand_staff() && !part.is_divisi() && part.bar_has_divisi(bar) {
            if staff == StaffId::Upper {
                &[Voice::One]
            } else {
                &[Voice::Two]
            }
        } else {
            &Voice::ALL
        };
    voices
        .iter()
        .map(|&voice| VoiceRegion {
            voice,
            start: 0,
            end: capacity,
        })
        .collect()
}

pub(super) fn finish(
    content: &mut MeasureLayout,
    part: &Part,
    staff: StaffId,
    bar: usize,
    rhythm: &Rhythm,
) {
    if !part.has_passage_divisi() {
        return;
    }
    if staff == StaffId::Upper {
        let y = content.top.min(-28.) - 20.;
        for (tick, mark) in part.divisi_changes_in(bar) {
            let Some((_, _, event, _)) = part.measures()[bar]
                .timed_iter()
                .find(|(_, t, _, _)| *t == tick)
            else {
                continue;
            };
            let id = ElementId {
                part: part.id(),
                staff: StaffId::Upper,
                measure: bar,
                event,
            };
            let x = rhythm.x(tick) - 5.;
            let width = if mark == Division::Divisi { 28. } else { 35. };
            content.techniques.push(TechniqueLabel {
                text: mark.label(),
                position: Point { x, y },
                element: id,
            });
            content.marks.push(MarkBox {
                mark: MarkRef::Division(id),
                glyph: None,
                center: Point {
                    x: x + width / 2.,
                    y: y - 5.,
                },
                half: (width / 2. + 3., 9.),
            });
            content.top = content.top.min(y - 16.);
        }
    }
}
