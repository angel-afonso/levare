// Levare landing page: the Escribir ribbon and the staff in the hero, which
// visitors write on and hear; the role sentence, the four ways to read a
// score, the visitor's platform and the entrances. The scroll scenes are in
// scenes.js.
(() => {
  const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const SVG = 'http://www.w3.org/2000/svg';

  // Replays a CSS animation on an element that already has its class.
  const replay = (el, cls) => {
    if (!el) return;
    el.classList.remove(cls);
    void el.getBoundingClientRect();
    el.classList.add(cls);
  };

  // --- platform: names, shortcut labels and the download that fits

  const platform = (() => {
    const ua = ((navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || navigator.userAgent || '').toLowerCase();
    if (ua.includes('win')) return 'win';
    if ((ua.includes('linux') || ua.includes('x11')) && !ua.includes('android')) return 'linux';
    return 'mac';
  })();
  const pc = platform !== 'mac';
  const OS_NAME = { mac: 'macOS', win: 'Windows', linux: 'Linux' };

  $$('[data-mac]').forEach((el) => { el.textContent = pc ? el.dataset.pc : el.dataset.mac; });
  $$('[data-os-name]').forEach((el) => { el.textContent = OS_NAME[platform]; });
  const card = $(`.dlc[data-os="${platform}"]`);
  $$('.dlc').forEach((c) => c.classList.toggle('on', c === card));
  const heroDownload = $('[data-dl]');
  if (heroDownload && card) heroDownload.href = $('.btn', card).href;
  $$('[data-js]').forEach((el) => { el.hidden = false; });

  // --- sound: a soft synthesized voice, started by the visitor's own click

  let audio = null;
  let voices = [];
  const context = () => {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    audio = audio || new AC();
    if (audio.state === 'suspended') audio.resume();
    return audio;
  };
  const STEP = { C: 0, D: 1, E: 2, F: 3, G: 4, A: 5, B: 6 };
  const SEMI = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };
  const freq = (n) => 440 * Math.pow(2, (12 * (n.o + 1) + SEMI[n.l] + (n.acc || 0) - 69) / 12);

  function tone(n, length, at, level = 0.2) {
    const ac = context();
    if (!ac) return;
    const t = at || ac.currentTime + 0.01;
    const body = ac.createOscillator();
    const air = ac.createOscillator();
    const gain = ac.createGain();
    const airGain = ac.createGain();
    body.type = 'triangle';
    air.type = 'sine';
    body.frequency.value = freq(n);
    air.frequency.value = freq(n) * 2;
    airGain.gain.value = 0.18;
    gain.gain.setValueAtTime(0, t);
    gain.gain.linearRampToValueAtTime(level, t + 0.04);
    gain.gain.linearRampToValueAtTime(level * 0.7, t + Math.min(0.3, length * 0.5));
    gain.gain.linearRampToValueAtTime(0, t + length);
    body.connect(gain);
    air.connect(airGain).connect(gain);
    gain.connect(ac.destination);
    body.start(t);
    air.start(t);
    body.stop(t + length + 0.05);
    air.stop(t + length + 0.05);
    voices.push(body, air);
  }

  function silence() {
    voices.forEach((v) => { try { v.stop(); } catch (e) { /* already stopped */ } });
    voices = [];
  }

  // --- the hero: the Escribir ribbon, the position strip and the staff

  const hero = $('.hero');
  const staff = $('#staff');
  const layer = $('[data-notes]', staff);
  const cursor = $('[data-cursor]', staff);
  const pill = $('[data-pill]');
  const pillWrap = $('[data-pill-wrap]');
  const readBar = $('[data-r-bar]');
  const readBeat = $('[data-r-beat]');
  const readVoice = $('[data-r-voice]');
  const readSel = $('[data-r-sel]');
  const NAME = { w: 'Redonda', h: 'Blanca', q: 'Negra', e: 'Corchea', s: 'Semicorchea', t: 'Fusa' };
  const LEN = { w: 4, h: 2, q: 1, e: 0.5, s: 0.25, t: 0.125 };
  const HEAD = { w: '', h: '', q: '', e: '', s: '', t: '' };
  const FLAG = { e: ['', ''], s: ['', ''], t: ['', ''] };
  const REST = { w: '', h: '', q: '', e: '', s: '', t: '' };
  const ACC = { '-1': '', 0: '', 1: '' };
  // Engraving: staff space 18 with the top line at y 80, two bars of 4/4.
  const S = 18, TOP = 80, X0 = 210, BAR = 590, BEAT = 130;
  const HW = 1.18 * S, STEM = 3.5 * S, BEAM = 0.5 * S, R = 0.17 * S;
  const DEMO = [['G', 'q'], ['A', 'e'], ['B', 'e'], ['C', 'q'], ['B', 'q'], ['A', 'h'], ['G', 'h']];

  const state = { entry: true, dur: 'q', dot: false, voice: 1, acc: null, octave: 0, message: '' };
  let notes = [];
  let playing = false;
  let demo = null;
  let timers = [];

  const dia = (n) => STEP[n.l] + 7 * n.o;
  const length = (n) => LEN[n.d] * (n.dot ? 1.5 : 1);
  const beats = (list) => list.reduce((sum, n) => sum + length(n), 0);

  // On a phone each bar gets its own system, one under the other.
  const phone = matchMedia('(max-width: 760px)');
  const SYSTEM = 190;
  const place = (bar, within, whole) => {
    const dx = phone.matches ? 0 : bar * BAR;
    const dy = phone.matches ? bar * SYSTEM : 0;
    return { x: X0 + dx + (whole ? 250 : 34 + within * BEAT), dy };
  };

  const make = (tag, attrs, text) => {
    const el = document.createElementNS(SVG, tag);
    Object.entries(attrs).forEach(([k, v]) => el.setAttribute(k, v));
    if (text) el.textContent = text;
    return el;
  };

  function frame() {
    const root = $('[data-frame]', staff);
    root.textContent = '';
    const systems = phone.matches ? [0, 1] : [0];
    const right = phone.matches ? 804 : 1392;
    const lines = make('g', { class: 'lines' });
    systems.forEach((k) => {
      for (let i = 0; i < 5; i++) lines.appendChild(make('path', { d: `M24 ${TOP + i * S + k * SYSTEM}H${right}` }));
    });
    root.appendChild(lines);
    const bars = phone.matches ? [[800, 0, ''], [790, SYSTEM, ''], [799, SYSTEM, ' end']] : [[800, 0, ''], [1380, 0, ''], [1389, 0, ' end']];
    bars.forEach(([x, dy, end]) => root.appendChild(make('path', { class: `bar${end}`, d: `M${x} ${TOP + dy}V${TOP + 4 * S + dy}` })));
    systems.forEach((k) => root.appendChild(make('text', { class: 'hbv clef', x: 40, y: TOP + 3 * S + k * SYSTEM }, '')));
    root.appendChild(make('text', { class: 'hbv clef', x: 128, y: TOP + S }, ''));
    root.appendChild(make('text', { class: 'hbv clef', x: 128, y: TOP + 3 * S }, ''));
    staff.setAttribute('viewBox', phone.matches ? '0 30 820 390' : '0 30 1400 200');
  }

  // The octave nearest the previous note (moved by the Octava buttons), kept
  // on the staff from D4 to G5.
  function octave(letter, prev) {
    const target = (prev ? dia(prev) : 34) + 7 * state.octave;
    let best = null;
    for (const o of [4, 5]) {
      const d = STEP[letter] + 7 * o;
      if (d < 29 || d > 39) continue;
      if (!best || Math.abs(d - target) < Math.abs(best.d - target)) best = { o, d };
    }
    return best.o;
  }

  function layout() {
    let beat = 0;
    const pos = notes.map((n) => {
      const bar = Math.floor(beat / 4);
      const within = beat - bar * 4;
      const { x, dy } = place(bar, within, n.d === 'w');
      beat += length(n);
      if (n.rest) {
        const y = TOP + (n.d === 'w' ? S : 2 * S) + dy;
        return { n, x, y, within, bar, dy, stem: null, flag: null };
      }
      const d = dia(n);
      return { n, x, y: TOP + 4 * S - (d - 30) * (S / 2) + dy, d, within, bar, dy, up: d < 34, stem: null, flag: null };
    });
    const beams = [];
    for (let i = 0; i < pos.length; i++) {
      const p = pos[i];
      const q = pos[i + 1];
      if (p.n.rest || p.n.d === 'w') continue;
      const pair = p.n.d === 'e' && !p.n.dot && q && !q.n.rest && q.n.d === 'e' && !q.n.dot && p.within % 1 === 0 && q.bar === p.bar;
      if (pair) {
        const up = (p.d + q.d) / 2 < 34;
        const x1 = up ? p.x + HW - 0.6 : p.x + 1.1;
        const x2 = up ? q.x + HW - 0.6 : q.x + 1.1;
        const by = up ? Math.min(p.y, q.y) - STEM : Math.max(p.y, q.y) + STEM;
        p.stem = [x1, up ? p.y - R : p.y + R, by];
        q.stem = [x2, up ? q.y - R : q.y + R, by];
        const top = up ? by : by - BEAM;
        beams.push({ last: i + 1, d: `M${x1 - 1.1} ${top}H${x2 + 1.1}V${top + BEAM}H${x1 - 1.1}Z` });
        i++;
        continue;
      }
      const sx = p.up ? p.x + HW - 0.6 : p.x + 1.1;
      const end = p.up ? p.y - STEM : p.y + STEM;
      p.stem = [sx, p.up ? p.y - R : p.y + R, end];
      if (FLAG[p.n.d]) p.flag = { x: sx - 1.1, y: end, g: FLAG[p.n.d][p.up ? 0 : 1] };
    }
    return { pos, beams, used: beat };
  }

  let positions = [];
  function render(fresh = -1) {
    const { pos, beams, used } = layout();
    positions = pos;
    layer.textContent = '';
    pos.forEach((p, i) => {
      const g = make('g', { class: i === fresh ? 'note in' : 'note' });
      if (p.n.rest) {
        g.appendChild(make('text', { class: 'hbv', x: p.x, y: p.y }, REST[p.n.d]));
      } else {
        g.appendChild(make('text', { class: 'hbv', x: p.x, y: p.y }, HEAD[p.n.d]));
        if (p.n.acc !== undefined && p.n.acc !== null) g.appendChild(make('text', { class: 'hbv', x: p.x - 26, y: p.y }, ACC[p.n.acc]));
        if (p.stem) g.appendChild(make('path', { class: 'stem', d: `M${p.stem[0]} ${p.stem[1]}V${p.stem[2]}` }));
        if (p.flag) g.appendChild(make('text', { class: 'hbv flag', x: p.flag.x, y: p.flag.y }, p.flag.g));
      }
      if (p.n.dot) {
        const onLine = !p.n.rest && (p.d - 30) % 2 === 0;
        const extra = p.n.d === 'w' ? 30 : HW + 6;
        g.appendChild(make('text', { class: 'hbv', x: p.x + extra, y: p.y - (onLine ? S / 2 : 0) }, ''));
      }
      layer.appendChild(g);
    });
    beams.forEach((b) => layer.appendChild(make('path', { class: b.last === fresh ? 'beam in' : 'beam', d: b.d })));
    const bar = Math.min(Math.floor(used / 4), 1);
    const full = used >= 8;
    const at = place(bar, used - bar * 4, false);
    cursor.style.transform = full ? 'translate(0, 0)' : `translate(${at.x - 10 + 50}px, ${at.dy}px)`;
    cursor.style.opacity = full || playing || !state.entry ? 0 : 1;
    staff.setAttribute('aria-label', notes.length
      ? `Pentagrama de flauta con ${notes.length} figuras`
      : 'Pentagrama de flauta vacío');
    readouts(used);
  }

  // The pill reads exactly as Levare's; messages go where the app shows the
  // selection, at the right of the position strip.
  function readouts(used = beats(notes)) {
    const figure = NAME[state.dur] + (state.dot ? ' con puntillo' : '');
    pill.textContent = state.entry ? `Escribiendo · ${figure} · Voz ${state.voice}` : 'Seleccionando · N para escribir';
    pillWrap.classList.toggle('idle', !state.entry);
    const at = Math.min(used, 7.999);
    readBar.textContent = `${Math.floor(at / 4) + 1} de 2`;
    readBeat.textContent = Math.floor(at % 4) + 1;
    readVoice.textContent = state.voice;
    const MSG = { inert: 'Esto se prueba en Levare; aquí, notas, silencios, puntillo y alteraciones', full: 'Los dos compases están llenos', nofit: `${figure} no cabe en el compás` };
    readSel.textContent = playing ? 'Reproduciendo' : MSG[state.message] || (notes.length ? '1 nota' : 'Nada seleccionado');
  }

  function add(n, quiet) {
    const used = beats(notes);
    const len = length(n);
    if (used >= 8) { state.message = 'full'; readouts(); return false; }
    if ((used % 4) + len > 4) { state.message = 'nofit'; readouts(); return false; }
    notes = notes.concat([n]);
    state.message = '';
    render(notes.length - 1);
    if (!quiet && !n.rest) tone(n, 0.35);
    return true;
  }

  const stopDemo = () => { clearInterval(demo); demo = null; hero.classList.remove('writing'); };

  function press(el) { replay(el, 'hit'); }

  function sync() {
    $$('[data-dur]').forEach((b) => { const on = b.dataset.dur === state.dur; b.classList.toggle('on', on); b.setAttribute('aria-pressed', on); });
    $$('[data-voice]').forEach((b) => { const on = Number(b.dataset.voice) === state.voice; b.classList.toggle('on', on); b.setAttribute('aria-pressed', on); });
    const entry = $('[data-entry]');
    entry.classList.toggle('on', state.entry);
    entry.setAttribute('aria-pressed', state.entry);
    const dot = $('[data-dot]');
    dot.classList.toggle('on', state.dot);
    dot.setAttribute('aria-pressed', state.dot);
    $$('[data-acc]').forEach((b) => { const on = state.acc !== null && Number(b.dataset.acc) === state.acc; b.classList.toggle('on', on); b.setAttribute('aria-pressed', on); });
    render();
  }

  function write(letter) {
    stopDemo();
    if (playing) stop();
    if (!state.entry) { state.entry = true; sync(); }
    const n = { l: letter, d: state.dur, dot: state.dot, o: octave(letter, notes.filter((x) => !x.rest).pop()) };
    if (state.acc !== null) n.acc = state.acc;
    if (add(n, false)) { state.acc = null; state.octave = 0; sync(); }
  }

  function rest() {
    stopDemo();
    if (!state.entry) return;
    add({ rest: true, d: state.dur, dot: state.dot }, true);
  }

  function undo() {
    stopDemo();
    notes = notes.slice(0, -1);
    state.message = '';
    render();
  }

  function playLabels() {
    hero.classList.toggle('playing', playing);
    $$('[data-play-label]').forEach((l) => { l.textContent = playing ? 'Detener' : 'Escuchar'; });
  }

  function stop() {
    timers.forEach(clearTimeout);
    timers = [];
    silence();
    playing = false;
    playLabels();
    render();
  }

  function ripple(p) {
    const ring = make('circle', { class: 'ripple', cx: p.x + HW / 2, cy: p.y, r: 16 });
    layer.appendChild(ring);
    setTimeout(() => ring.remove(), 900);
  }

  function play() {
    if (playing) { stop(); return; }
    stopDemo();
    if (!notes.length) return;
    const ac = context();
    const spb = 60 / 96;
    const t0 = ac ? ac.currentTime + 0.08 : 0;
    let t = 0;
    playing = true;
    playLabels();
    render();
    const groups = $$('.note', layer);
    notes.forEach((n, i) => {
      const len = length(n) * spb;
      if (ac && !n.rest) tone(n, len * 0.95, t0 + t);
      timers.push(setTimeout(() => {
        groups.forEach((g) => g.classList.remove('now'));
        if (groups[i]) groups[i].classList.add('now');
        if (!reduced && !n.rest) ripple(positions[i]);
      }, t * 1000 + 80));
      t += len;
    });
    timers.push(setTimeout(stop, t * 1000 + 250));
  }

  // Ribbon controls the web can play; the rest answer like a pressed key.
  const clickOn = (sel, fn) => $$(sel).forEach((b) => b.addEventListener('click', (e) => { press(b); fn(b, e); }));
  clickOn('[data-dur]', (b) => { stopDemo(); state.dur = b.dataset.dur; state.message = ''; sync(); });
  clickOn('[data-voice]', (b) => { state.voice = Number(b.dataset.voice); sync(); });
  clickOn('[data-entry]', () => { stopDemo(); state.entry = !state.entry; sync(); });
  clickOn('[data-dot]', () => { state.dot = !state.dot; sync(); });
  clickOn('[data-rest]', () => rest());
  clickOn('[data-inert]', () => { state.message = 'inert'; readouts(); });
  clickOn('[data-acc]', (b) => { const a = Number(b.dataset.acc); state.acc = state.acc === a ? null : a; sync(); });
  clickOn('[data-oct]', (b) => { state.octave = Math.max(-1, Math.min(1, state.octave + Number(b.dataset.oct))); });
  clickOn('[data-note]', (b) => write(b.dataset.note));
  clickOn('[data-undo]', () => undo());
  clickOn('[data-clear]', () => { stopDemo(); if (playing) stop(); notes = []; state.message = ''; render(); });
  clickOn('[data-play]', () => play());

  // Typing on the staff, only while it has the focus: Levare's own keys.
  staff.setAttribute('tabindex', '0');
  staff.addEventListener('pointerdown', () => staff.focus({ preventScroll: true }));
  staff.addEventListener('keydown', (e) => {
    if (e.metaKey || e.ctrlKey) return;
    const k = e.key;
    const keyDur = { 6: 'w', 5: 'h', 4: 'q', 3: 'e', 2: 's', 1: 't' }[k];
    const keyAcc = { 7: 0, 8: 1, 9: -1 }[k];
    let target = null;
    if (e.altKey && (e.code === 'Digit1' || e.code === 'Digit2')) { state.voice = Number(e.code.slice(-1)); target = $(`[data-voice="${state.voice}"]`); sync(); }
    else if (e.altKey) return;
    else if (/^[a-g]$/i.test(k) && state.entry) { write(k.toUpperCase()); target = $(`[data-note="${k.toUpperCase()}"]`); }
    else if (k === 'n' || k === 'N') { stopDemo(); state.entry = !state.entry; target = $('[data-entry]'); sync(); }
    else if (keyDur) { stopDemo(); state.dur = keyDur; state.message = ''; target = $(`[data-dur="${keyDur}"]`); sync(); }
    else if (keyAcc !== undefined) { state.acc = state.acc === keyAcc ? null : keyAcc; target = $(`[data-acc="${keyAcc}"]`); sync(); }
    else if (k === '.') { state.dot = !state.dot; target = $('[data-dot]'); sync(); }
    else if (k === '0') { rest(); target = $('[data-rest]'); }
    else if (k === 'Backspace' || k === 'Delete') { undo(); target = $('[data-undo]'); }
    else if (k === ' ') { play(); }
    else return;
    e.preventDefault();
    press(target);
  });

  if (phone.matches) frame();
  phone.addEventListener('change', () => { frame(); render(); });

  // The phrase writes itself once the staff has been drawn. Without motion
  // the engraved phrase in the page simply stays.
  const phrase = () => DEMO.reduce((list, [l, d]) => list.concat([{ l, d, o: octave(l, list[list.length - 1]) }]), []);
  if (reduced) {
    notes = phrase();
    render();
  } else {
    const typed = phrase();
    notes = [];
    render();
    hero.classList.add('writing');
    setTimeout(() => {
      if (notes.length || !hero.classList.contains('writing')) return;
      let i = 0;
      demo = setInterval(() => {
        if (i >= typed.length) { stopDemo(); return; }
        add(typed[i], true);
        i++;
      }, 430);
    }, 1300);
  }

  // --- tabs that swap panes: the roles

  function tabs(buttonSel, paneSel, key, onPick) {
    const buttons = $$(buttonSel);
    const pick = (b) => {
      const v = b.dataset[key];
      buttons.forEach((x) => {
        const on = x === b;
        x.classList.toggle('on', on);
        x.setAttribute(x.getAttribute('role') === 'tab' ? 'aria-selected' : 'aria-pressed', on);
      });
      $$(paneSel).forEach((p) => { p.hidden = p.dataset[key] !== v; });
      if (onPick) onPick(b, v);
    };
    buttons.forEach((b) => b.addEventListener('click', () => pick(b)));
    return pick;
  }

  const verb = $('[data-verb]');
  tabs('.verb[data-role]', '[data-role-pane]', 'role', (b) => { verb.textContent = b.textContent; replay(verb, 'swap'); });

  // --- one score, four ways to read it

  const lens = $('.lens');
  let excerptTimers = [];
  const excerptButton = $('[data-excerpt]');
  const excerptLabel = $('[data-excerpt-label]');

  function stopExcerpt() {
    excerptTimers.forEach(clearTimeout);
    excerptTimers = [];
    silence();
    excerptLabel.textContent = 'Escuchar estos compases';
    lens.classList.remove('sounding');
  }

  $$('[data-lens-tab]').forEach((b) => b.addEventListener('click', () => {
    lens.dataset.lens = b.dataset.lensTab;
    $$('[data-lens-tab]').forEach((x) => {
      x.parentElement.classList.toggle('on', x === b);
      x.setAttribute('aria-selected', x === b);
    });
    if (excerptTimers.length) stopExcerpt();
  }));

  // Bars 7 and 8 as they sound: the clarinet a tone below its written notes.
  if (excerptButton) excerptButton.addEventListener('click', () => {
    if (excerptTimers.length) { stopExcerpt(); return; }
    const ac = context();
    if (!ac) return;
    const spb = 60 / 72;
    const t0 = ac.currentTime + 0.08;
    const clarinet = [['G', 4, 1], ['B', 4, 1], ['A', 4, 2], ['G', 4, 4]];
    const cello = [['G', 2, 2], ['D', 3, 2], ['G', 2, 2], ['B', 1, 2, -1]];
    [[clarinet, 0.18], [cello, 0.22]].forEach(([line, level]) => {
      let t = 0;
      line.forEach(([l, o, b, acc]) => {
        tone({ l, o, acc }, b * spb * 0.95, t0 + t, level);
        t += b * spb;
      });
    });
    excerptLabel.textContent = 'Detener';
    lens.classList.add('sounding');
    excerptTimers.push(setTimeout(stopExcerpt, 8 * spb * 1000 + 300));
  });

  // --- entrances (the scroll scenes live in scenes.js)

  if ('IntersectionObserver' in window && !reduced) {
    document.documentElement.classList.add('rv-on');
    const io = new IntersectionObserver((entries) => {
      entries.forEach((en) => {
        if (!en.isIntersecting) return;
        en.target.classList.add('in');
        io.unobserve(en.target);
      });
    }, { threshold: 0.12, rootMargin: '0px 0px -6% 0px' });
    $$('.rv, .mark').forEach((el) => io.observe(el));
  }
})();
