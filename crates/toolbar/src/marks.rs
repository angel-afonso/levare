//! The commands that write each catalog mark. Ribbon, menus, inspector and
//! the app's dispatch all go through this one table.
use score_core::{
    Arpeggio as R, Articulation as A, ConnectionKind as K, Dynamic as D, MarkKind,
    PlayingTechnique as T,
};
use score_keymap::{CommandId, id};

pub const fn command(kind: MarkKind) -> CommandId {
    match kind {
        MarkKind::Division(score_core::Division::Divisi) => id::DIVISI,
        MarkKind::Division(score_core::Division::Unison) => id::UNISON,
        MarkKind::Arpeggio(R::Normal) => id::ARPEGGIO,
        MarkKind::Arpeggio(R::Up) => id::ARPEGGIO_UP,
        MarkKind::Arpeggio(R::Down) => id::ARPEGGIO_DOWN,
        MarkKind::Articulation(A::Staccato) => id::STACCATO,
        MarkKind::Articulation(A::Tenuto) => id::TENUTO,
        MarkKind::Articulation(A::Accent) => id::ACCENT,
        MarkKind::Articulation(A::Marcato) => id::MARCATO,
        MarkKind::Fermata => id::FERMATA,
        MarkKind::Dynamic(D::Ppp) => id::DYNAMIC_PPP,
        MarkKind::Dynamic(D::Pp) => id::DYNAMIC_PP,
        MarkKind::Dynamic(D::P) => id::DYNAMIC_P,
        MarkKind::Dynamic(D::Mp) => id::DYNAMIC_MP,
        MarkKind::Dynamic(D::Mf) => id::DYNAMIC_MF,
        MarkKind::Dynamic(D::F) => id::DYNAMIC_F,
        MarkKind::Dynamic(D::Ff) => id::DYNAMIC_FF,
        MarkKind::Dynamic(D::Fff) => id::DYNAMIC_FFF,
        MarkKind::Technique(T::Pizzicato) => id::PIZZICATO,
        MarkKind::Technique(T::Arco) => id::ARCO,
        MarkKind::Harmonic(score_core::Harmonic::Natural) => id::HARMONIC_NATURAL,
        MarkKind::Harmonic(score_core::Harmonic::Artificial) => id::HARMONIC_ARTIFICIAL,
        MarkKind::Connection(K::Tie) => id::TIE,
        MarkKind::Connection(K::Slur) => id::SLUR,
        MarkKind::Connection(K::Crescendo) => id::CRESCENDO,
        MarkKind::Connection(K::Diminuendo) => id::DIMINUENDO,
    }
}

/// The mark a command writes, if it writes one.
pub fn kind(command: CommandId) -> Option<MarkKind> {
    MarkKind::ALL
        .into_iter()
        .find(|kind| self::command(*kind) == command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_keymap::{Registry, Status};

    #[test]
    fn every_mark_has_its_own_ready_command_named_after_it() {
        let registry = Registry::builtin();
        let mut seen = std::collections::HashSet::new();
        for mark in MarkKind::ALL {
            let command = command(mark);
            assert!(seen.insert(command), "{command}");
            assert_eq!(kind(command), Some(mark));
            let spec = registry.get(command.as_str()).unwrap();
            assert_eq!(spec.status, Status::Ready, "{command}");
            let label = spec.label.to_lowercase();
            assert!(
                label.contains(&mark.label().to_lowercase())
                    || label.contains(&mark.name().to_lowercase()),
                "{}: {}",
                spec.label,
                mark.label()
            );
        }
        assert_eq!(kind(id::COPY), None);
    }
}
