//! Presentation only: event identity, musical time and the document stay intact.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    Page,
    #[default]
    Continuous,
    SingleSystem,
}
impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Page => "Página",
            Self::Continuous => "Continua",
            Self::SingleSystem => "Pentagrama",
        }
    }
}

#[derive(Clone, Debug)]
pub struct System {
    pub first_measure: usize,
    pub last_measure: usize,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Debug)]
pub struct Page {
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub first_measure: usize,
    pub last_measure: usize,
}

const PAGE_WIDTH: f32 = 1000.;
// Printed staff sizes are independent of display zoom.
const ENSEMBLE_PAGE_WIDTH: f32 = 1600.;
const PART_PAGE_WIDTH: f32 = 1280.;
const PAGE_HEIGHT: f32 = PAGE_WIDTH * std::f32::consts::SQRT_2;
const HEADER: f32 = 160.;
const MARGIN: f32 = 56.;
const PAGE_GAP: f32 = 32.;

impl LayoutEngine {
    pub fn layout_view(&mut self, score: &Score, width: f32, mode: ViewMode) -> Layout {
        self.layout_presentation(score, width, mode, false)
    }

    /// An extracted instrumental part: compact print scale, heading instead
    /// of repeated instrument names, with divisi labels and grand-staff names preserved.
    pub fn layout_part(&mut self, score: &Score, width: f32, mode: ViewMode) -> Layout {
        self.layout_presentation(score, width, mode, score.parts().len() == 1)
    }

    fn layout_presentation(
        &mut self,
        score: &Score,
        width: f32,
        mode: ViewMode,
        part: bool,
    ) -> Layout {
        let orchestra_page = !part && score.parts().len() >= 12;
        let page_width = if orchestra_page {
            self.orchestra_page
                .as_ref()
                .filter(|(cached, _)| cached == score)
                .map_or(ENSEMBLE_PAGE_WIDTH, |(_, width)| *width)
        } else if part {
            PART_PAGE_WIDTH
        } else if score.parts().len() >= 3 {
            ENSEMBLE_PAGE_WIDTH
        } else {
            PAGE_WIDTH
        };
        let mut layout = self.layout_with_spacing(
            score,
            if mode == ViewMode::Page {
                page_width
            } else {
                width
            },
            true,
            mode == ViewMode::SingleSystem,
            None,
            part,
        );
        layout.mode = mode;
        spacing::compact(&mut layout, &self.metrics);
        // A large orchestra needs a proportional writing area: extending only
        // the height would shrink a narrow column into the center of the PDF.
        if mode == ViewMode::Page && orchestra_page {
            loop {
                let tallest = layout
                    .systems
                    .iter()
                    .map(|s| s.bottom - s.top)
                    .fold(0., f32::max);
                let orchestra_width = (tallest + HEADER + MARGIN) / std::f32::consts::SQRT_2;
                if orchestra_width <= layout.width + 0.5 {
                    break;
                }
                // Merging bars can combine high and low registers into a taller
                // system. Resolve the final width before retaining its geometry.
                layout = self.layout_with_spacing(score, orchestra_width, true, false, None, false);
                layout.mode = mode;
                spacing::compact(&mut layout, &self.metrics);
            }
        }
        if mode == ViewMode::Page && orchestra_page {
            self.orchestra_page = Some((score.clone(), layout.width));
        }
        if mode == ViewMode::Page {
            layout.paginate(16., MARGIN + if part { 16. } else { 0. });
        } else {
            let mut next_top = HEADER;
            let shifts: Vec<_> = layout
                .systems
                .iter()
                .map(|system| {
                    let shift = next_top - system.top;
                    next_top += system.bottom - system.top + 16.;
                    shift
                })
                .collect();
            layout.move_systems(&shifts);
            layout.height = (layout.systems.last().unwrap().bottom + MARGIN).max(
                if mode == ViewMode::Continuous {
                    PAGE_HEIGHT
                } else {
                    0.
                },
            );
            layout.pages.push(Page {
                top: 0.,
                width: layout.width,
                height: layout.height,
                first_measure: 0,
                last_measure: score.measures().len() - 1,
            });
        }
        // Connections must be segmented at the final system/page positions.
        layout.piano_arpeggios =
            expression::piano_arpeggios(score, &layout.measures, &self.metrics);
        let index = expression::PlacedIndex::new(&layout.measures);
        let curves = expression::curves(score, &index, &self.metrics);
        let hairpins = expression::hairpins(score, &index, self.metrics.hairpin);
        (layout.curves, layout.hairpins) = (curves, hairpins);
        offsets::apply(score, &mut layout);
        offsets::apply_bar_marks(score, &mut layout);
        layout
    }

    /// Re-engrave tentative pitches on the original rhythmic grid and staves.
    /// The caller keeps durations, voices and instrumentation unchanged. Space
    /// for new ink is reserved only after committing, so high notes follow the
    /// pointer instead of pushing their staff in the opposite direction.
    pub fn layout_pitch_preview(&mut self, score: &Score, base: &Layout) -> Layout {
        let mut layout = self.layout_with_spacing(
            score,
            base.width,
            true,
            base.mode == ViewMode::SingleSystem,
            Some(base),
            false,
        );
        // The preview pins staff positions to the displayed score. Reuse its
        // system ownership while recomputing local indication clearance.
        layout.systems.clone_from(&base.systems);
        spacing::compact(&mut layout, &self.metrics);
        layout.scale = base.scale;
        layout.mode = base.mode;
        layout.pages.clone_from(&base.pages);
        layout.systems.clone_from(&base.systems);
        layout.labels.clone_from(&base.labels);
        layout.tempos.clone_from(&base.tempos);
        layout.rehearsals.clone_from(&base.rehearsals);
        layout.endings.clone_from(&base.endings);
        layout.navigation.clone_from(&base.navigation);
        layout.width = base.width;
        layout.height = base.height;
        offsets::apply(score, &mut layout);
        layout
    }
}

impl Layout {
    pub fn page_for_measure(&self, measure: usize) -> Option<usize> {
        self.pages
            .iter()
            .position(|page| (page.first_measure..=page.last_measure).contains(&measure))
    }

    fn paginate(&mut self, system_gap: f32, continuation_top: f32) {
        // An indivisible system may be wider/taller than a normal sheet (dense
        // bar or large orchestra). Enlarge every sheet consistently; never crop
        // ink, split piano hands or stretch the musical glyphs.
        let width = self.width.max(PAGE_WIDTH);
        let tallest = self
            .systems
            .iter()
            .map(|s| s.bottom - s.top)
            .fold(0., f32::max);
        let height = (width * std::f32::consts::SQRT_2).max(tallest + HEADER + MARGIN);
        let mut page_top = 0.;
        let mut next_top = HEADER;
        let mut first_measure = 0;
        let mut shifts = Vec::new();
        for index in 0..self.systems.len() {
            let system_height = self.systems[index].bottom - self.systems[index].top;
            if next_top + system_height > page_top + height - MARGIN && index > 0 {
                self.pages.push(Page {
                    top: page_top,
                    width,
                    height,
                    first_measure,
                    last_measure: self.systems[index - 1].last_measure,
                });
                page_top += height + PAGE_GAP;
                next_top = page_top + continuation_top;
                first_measure = self.systems[index].first_measure;
            }
            let delta = next_top - self.systems[index].top;
            shifts.push(delta);
            next_top += system_height + system_gap;
        }
        self.pages.push(Page {
            top: page_top,
            width,
            height,
            first_measure,
            last_measure: self.systems.last().unwrap().last_measure,
        });
        self.move_systems(&shifts);
        self.width = width;
        self.height = page_top + height;
    }

    fn move_systems(&mut self, shifts: &[f32]) {
        // Resolve every object against the original systems before moving any
        // system: translated ink can overlap a later system's old coordinates.
        let shift_at = |y: f32| {
            let index = self
                .systems
                .partition_point(|s| s.top <= y)
                .saturating_sub(1);
            if y < self.systems[index].bottom {
                shifts[index]
            } else {
                0.
            }
        };
        let mut measure_shifts = vec![0.; self.systems.last().unwrap().last_measure + 1];
        for (system, shift) in self.systems.iter().zip(shifts) {
            measure_shifts[system.first_measure..=system.last_measure].fill(*shift);
        }
        for measure in &mut self.measures {
            measure.origin.y += measure_shifts[measure.index];
        }
        for mark in &mut self.tempos {
            mark.position.y += shift_at(mark.position.y);
        }
        for label in &mut self.rehearsals {
            let shift = measure_shifts[label.measure];
            label.position.y += shift;
            for line in &mut label.frame {
                line.from.y += shift;
                line.to.y += shift;
            }
        }
        for label in &mut self.navigation {
            label.position.y += measure_shifts[label.measure];
        }
        for ending in &mut self.endings {
            let shift = shift_at(ending.lines[0].from.y);
            ending.label_position.y += shift;
            for line in &mut ending.lines {
                line.from.y += shift;
                line.to.y += shift;
            }
        }
        for label in &mut self.labels {
            label.position.y += shift_at(label.position.y);
        }
        for line in &mut self.connectors {
            let shift = shift_at(line.from.y);
            line.from.y += shift;
            line.to.y += shift;
        }
        for brace in &mut self.braces {
            let shift = shift_at(brace.top);
            brace.top += shift;
            brace.bottom += shift;
        }
        for bracket in &mut self.brackets {
            let shift = shift_at(bracket.top);
            bracket.top += shift;
            bracket.bottom += shift;
        }
        for (system, shift) in self.systems.iter_mut().zip(shifts) {
            system.top += shift;
            system.bottom += shift;
        }
    }
}
