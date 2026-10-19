//! Composition root: optional bank registration is isolated from editor/transport.
use score_platform::sound_engine::SoundBank;
use std::sync::Arc;

pub fn discover() -> Vec<Arc<dyn SoundBank>> {
    #[cfg(feature = "vsco")]
    {
        score_sound_vsco::VscoPack::discover()
            .map(|bank| vec![Arc::new(bank) as Arc<dyn SoundBank>])
            .unwrap_or_default()
    }
    #[cfg(not(feature = "vsco"))]
    {
        Vec::new()
    }
}
