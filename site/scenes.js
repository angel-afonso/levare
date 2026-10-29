// Levare landing page, scroll scenes: the app's window opening and walking
// through its parts, the four beats led by the scroll, and the quieter
// scroll-linked moves around them. Built on assets/scenes/stage.js. Without
// GSAP, on a phone or with reduced motion, every section shows its finished
// composition and the beats answer to clicks.
(() => {
  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  if (!window.Stage) return;

  // The window lies tilted on the desk, straightens to fill the screen, and
  // then lights up its parts one by one, each with what it is for.
  Scenes['app-tour'] = (root, opts = {}) => Stage.scene(root, {
    hold: opts.hold ?? 3,
    scrub: 0.5,
    phone: 'rest',
    build(tl, { q }) {
      // The heading leaves before the window grows into its place.
      tl.to(q('.tour-head'), { autoAlpha: 0, yPercent: -30, duration: 0.07 }, 0)
        .fromTo(q('.tour-shot'), { scale: 0.72, rotateX: 20, yPercent: 14 }, { scale: 1, rotateX: 0, yPercent: 0, duration: 0.22, ease: 'power2.out' }, 0.05);
      const zones = q('.zone');
      const notes = q('.tour-list li');
      zones.forEach((zone, i) => {
        const at = 0.28 + i * 0.14;
        tl.fromTo(zone, { autoAlpha: 0 }, { autoAlpha: 1, duration: 0.03 }, at)
          .fromTo(notes[i], { autoAlpha: 0, y: 18 }, { autoAlpha: 1, y: 0, duration: 0.04, ease: 'power2.out' }, at + 0.01);
        if (i < zones.length - 1) tl.to([zone, notes[i]], { autoAlpha: 0, duration: 0.03 }, at + 0.12);
      });
    },
  });

  // The four beats: the scroll conducts, one beat for each quarter of the hold.
  Scenes['four-beats'] = (root, opts = {}) => Stage.scene(root, {
    hold: opts.hold ?? 4,
    beats: 4,
    scrub: true,
    phone: 'rest',
    build(tl, { q }) {
      tl.fromTo(q('.beat-progress i'), { scaleX: 0 }, { scaleX: 1, duration: 1 }, 0);
    },
  });

  const four = $('#compas');
  const beats = $$('.beat', four);
  const syncBeats = () => {
    const now = Number(four.dataset.beat || 0);
    beats.forEach((b, i) => {
      b.classList.toggle('on', i === now);
      b.setAttribute('aria-selected', i === now);
    });
  };
  new MutationObserver(syncBeats).observe(four, { attributes: true, attributeFilter: ['data-beat'] });
  beats.forEach((b, i) => b.addEventListener('click', () => {
    if (!four.classList.contains('is-live')) { four.dataset.beat = String(i); return; }
    const top = four.getBoundingClientRect().top + window.scrollY;
    const range = four.offsetHeight - window.innerHeight;
    window.scrollTo({ top: top + range * (i + 0.5) / 4, behavior: 'smooth' });
  }));

  Stage.init();

  // Quiet moves, scrubbed by the scroll outside the scenes.
  if (!Stage.live) return;
  gsap.matchMedia().add('(prefers-reduced-motion: no-preference)', () => {
    const scrub = (trigger, start, end) => ({ trigger, start, end, scrub: 0.6 });
    // The headline's two lines part as the hero leaves.
    gsap.to('.hero .ln:not(.l2)', { xPercent: -7, ease: 'none', scrollTrigger: scrub('.hero', 'top top', 'bottom top') });
    gsap.to('.hero .ln.l2', { xPercent: 7, ease: 'none', scrollTrigger: scrub('.hero', 'top top', 'bottom top') });
    // The score of the four ways settles onto the desk as it arrives.
    gsap.fromTo('.lens-grid', { '--rise': '90px', '--size': 0.93 }, { '--rise': '0px', '--size': 1, ease: 'none', scrollTrigger: scrub('.lens', 'top bottom', 'top 30%') });
    // The roadmap: the playhead crosses what is already in Levare and stops at today,
    // writing each note of what already shipped as it passes.
    const road = gsap.timeline({ scrollTrigger: scrub('.road-staff', 'top 85%', 'top 30%') });
    road.fromTo('.rs-head rect', { attr: { x: -2 } }, { attr: { x: 798 }, duration: 1, ease: 'none' }, 0);
    $$('.rnote.done').forEach((note) => {
      const at = Math.max(0, (Number(note.getAttribute('x')) - 30) / 800);
      road.fromTo(note, { autoAlpha: 0, y: -14 }, { autoAlpha: 1, y: 0, duration: 0.05 }, at);
    });
    road.fromTo('.rnote.next', { autoAlpha: 0 }, { autoAlpha: 0.75, duration: 0.2, stagger: 0.02 }, 0.8);
    // The download cards arrive one after another.
    gsap.fromTo('.dlc', { '--lift': '120px' }, { '--lift': '0px', ease: 'none', stagger: 0.12, scrollTrigger: scrub('.dlg', 'top bottom', 'top 40%') });
    // The name closes up like a downbeat.
    gsap.fromTo('.word', { letterSpacing: '0.1em', '--rise': '60px' }, { letterSpacing: '-0.06em', '--rise': '0px', ease: 'none', scrollTrigger: scrub('.foot', 'top bottom', 'bottom bottom') });
  });
})();
