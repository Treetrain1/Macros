import './style.css';
import { iconEl, setBtnContent, INSTRUCTION_TYPE_ICONS, INSTRUCTION_TYPE_LABELS } from './icons.js';
import { dropdown, closeAllDropdowns } from './dropdown.js';

// Tauri v2 API
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ─── App state (full snapshot from backend) ──────────────────────────────────
let state = {};
let appVersion = '';

// One-shot animation tracking: only shake/enter-animate on the render where a
// field first goes invalid / a warning first appears, not on every rebuild
// while it stays that way (rows are fully recreated on every state push).
let prevInvalidKeys = new Set();
let prevPortInvalid = false;
let prevWarnings = { grab: false, emulator: false };

// ─── Custom dropdowns (persistent instances, mounted once in setupStaticListeners) ──
let macroDropdown = null;
let prevMacroOptionsKey = '';
let pendingMacroDropdown = null;

// ─── Canvas (Scratch-like strand layout) ──────────────────────────────────────
// Strand x/y from the backend are canvas-space coordinates that can go
// negative; canvas-inner is sized to the strands' bounding box each render
// (see renderCanvas), offset by this padding so cards never touch the edge.
const CANVAS_PAD = 400;
// Bounds always extend at least this far past the origin in every direction,
// so there's room to pan around the root strand even when it's the only
// thing on the canvas.
const ROOT_MARGIN = 1000;
let currentMacroId = null; // used to detect "switched macro" so we can re-center the canvas scroll

// ─── Theme ────────────────────────────────────────────────────────────────
let currentTheme = document.documentElement.dataset.theme === 'light' ? 'light' : 'dark';

function setTheme(theme) {
    currentTheme = theme;
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem('macros-theme', theme); } catch (_) {}
    updateThemeToggleIcon();
}

function updateThemeToggleIcon() {
    const btn = document.getElementById('theme-toggle-btn');
    if (!btn) return;
    setBtnContent(btn, { icon: currentTheme === 'light' ? 'moon' : 'sun' });
    const label = currentTheme === 'light' ? 'Switch to dark theme' : 'Switch to light theme';
    btn.title = label;
    btn.setAttribute('aria-label', label);
}

// ─── Initialisation ──────────────────────────────────────────────────────────
async function init() {
    // Always wire up buttons first so they work regardless of state-load outcome.
    setupStaticListeners();

    try {
        appVersion = await window.__TAURI__.app.getVersion();
    } catch (_) {}
    try {
        state = await invoke('get_state');
    } catch (e) {
        console.error('Failed to get initial state:', e);
    }
    render(state);
    try {
        await listen('state-updated', evt => {
            state = evt.payload;
            render(state);
        });
    } catch (e) {
        console.error('Failed to subscribe to state updates:', e);
    }
}

// ─── Keyboard capture (key instructions + hotkey combo capture) ──────────────
document.addEventListener('keydown', async e => {
    if (state.key_capture != null) {
        e.preventDefault();
        await invoke('key_capture_event', { code: e.code, key: e.key });
        return;
    }
    if (state.combo_capture != null) {
        e.preventDefault();
        if (e.key === 'Escape') { await invoke('cancel_combo_capture'); return; }
        if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;
        const modifiers = (e.ctrlKey ? 1 : 0) | (e.shiftKey ? 2 : 0) |
                          (e.altKey  ? 4 : 0) | (e.metaKey  ? 8 : 0);
        await invoke('combo_capture_event', { code: e.code, modifiers });
    }
});

// ─── Master render ────────────────────────────────────────────────────────────
function render(s) {
    closeAllDropdowns();
    const onMain = s.page !== 'Settings';
    document.getElementById('main-page').classList.toggle('page-hidden', !onMain);
    document.getElementById('settings-page').classList.toggle('page-hidden', onMain);
    try {
        if (onMain) renderMain(s);
        else renderSettings(s);
    } catch (e) {
        console.error('render error:', e);
    }
}

// ═══ Main Page ════════════════════════════════════════════════════════════════

function renderMain(s) {
    renderMacroSelector(s);
    renderRunControls(s);
    renderEditor(s);
}

function renderMacroSelector(s) {
    const newOptionsKey = (s.macro_names ?? []).join('|');
    if (newOptionsKey !== prevMacroOptionsKey) {
        macroDropdown.ddSetOptions((s.macro_names ?? []).map((name, idx) => ({ value: String(idx), label: name })));
        prevMacroOptionsKey = newOptionsKey;
    }

    const selectedVal = s.macro_selected != null ? String(s.macro_selected) : '';
    if (macroDropdown.ddValue !== selectedVal) macroDropdown.ddValue = selectedVal;

    const removeBtn = document.getElementById('remove-macro-btn');
    const hasSelected = s.macro_selected != null;
    removeBtn.disabled = !hasSelected;
    removeBtn.classList.toggle('confirm-armed', s.confirm_remove_macro);
    setBtnContent(removeBtn, s.confirm_remove_macro
        ? { icon: 'alert-triangle', text: `Delete (${s.confirm_remove_macro_remaining_secs ?? 3}s)?` }
        : { icon: 'trash', text: 'Delete' });
}

function renderRunControls(s) {
    const runBtn = document.getElementById('run-macro-btn');
    const loopCheck = document.getElementById('loop-mode-check');
    const recordBtn = document.getElementById('record-btn');

    runBtn.disabled = s.macro_selected == null;
    setBtnContent(runBtn, s.loop_mode_enabled
        ? { icon: 'repeat', text: 'Start loop' }
        : { icon: 'play', text: 'Run macro' });

    if (loopCheck.checked !== s.loop_mode_enabled) {
        loopCheck.checked = s.loop_mode_enabled;
    }

    const phase = s.recording_phase;
    if (phase.phase === 'Countdown') {
        setBtnContent(recordBtn, { icon: 'pause', text: `Recording in ${phase.countdown}s…` });
        recordBtn.className = 'btn-record btn-record-countdown';
        recordBtn.disabled = false;
    } else if (phase.phase === 'Active') {
        setBtnContent(recordBtn, { icon: 'square', text: 'Stop recording (Esc)' });
        recordBtn.className = 'btn-active-record';
        recordBtn.disabled = false;
    } else {
        setBtnContent(recordBtn, { icon: 'circle', text: 'Record' });
        recordBtn.className = 'btn-record';
        recordBtn.disabled = s.macro_selected == null;
    }
}

// ─── Instruction Editor ───────────────────────────────────────────────────────

function renderEditor(s) {
    const editorEl = document.getElementById('macro-editor');
    const emptyStateEl = document.getElementById('no-macro-state');
    if (s.current_macro == null) {
        editorEl.classList.add('hidden');
        emptyStateEl.classList.remove('hidden');
        document.getElementById('recording-overlay').classList.add('hidden');
        editorEl.classList.remove('recording');
        currentMacroId = null;
        return;
    }
    editorEl.classList.remove('hidden');
    emptyStateEl.classList.add('hidden');

    // Toggle recording overlay
    const isRecording = s.recording_phase?.phase === 'Active';
    editorEl.classList.toggle('recording', isRecording);
    document.getElementById('recording-overlay').classList.toggle('hidden', !isRecording);

    // Title
    const titleInput = document.getElementById('macro-title');
    if (document.activeElement !== titleInput) {
        titleInput.value = s.current_macro.name;
    }

    // Toolbar buttons
    document.getElementById('undo-btn').disabled = !s.can_undo;
    document.getElementById('redo-btn').disabled = !s.can_redo;
    const clearBtn = document.getElementById('clear-instructions-btn');
    clearBtn.classList.toggle('confirm-armed', s.confirm_clear_instructions);
    setBtnContent(clearBtn, s.confirm_clear_instructions
        ? { icon: 'alert-triangle', text: `Confirm clear (${s.confirm_clear_instructions_remaining_secs ?? 5}s)?` }
        : { icon: 'trash', text: 'Clear instructions' });

    renderCanvas(s);
}

function saveFocusedInput() {
    const el = document.activeElement;
    if (!el || el.dataset.strand === undefined || el.dataset.ix === undefined || el.dataset.field === undefined) return null;
    return {
        strand: el.dataset.strand,
        ix: el.dataset.ix,
        field: el.dataset.field,
        start: el.selectionStart,
        end: el.selectionEnd,
    };
}

function restoreFocusedInput(saved) {
    if (!saved) return;
    const inner = document.getElementById('canvas-inner');
    if (!inner) return;
    const el = inner.querySelector(`[data-strand="${cssEscape(saved.strand)}"][data-ix="${saved.ix}"][data-field="${saved.field}"]`);
    if (el) {
        el.focus();
        if (saved.start != null) {
            el.setSelectionRange(saved.start, saved.end);
        }
    }
}

function cssEscape(s) {
    return String(s).replace(/[^a-zA-Z0-9_-]/g, '\\$&');
}

const ROOT_ID = 'root';

// ═══ Canvas (Scratch-like strand layout) ═══════════════════════════════════

// Bounds of the last render, needed to convert pointer/client coordinates
// back into canvas (strand.x/y) space when a drag ends on empty canvas.
let lastBounds = { minX: 0, minY: 0 };

// ── Zoom (ctrl+scroll) ──────────────────────────────────────────────────────
// canvas-inner stays laid out at unscaled canvas-space coordinates and is
// visually scaled with a CSS transform; canvas-sizer's box is set to the
// zoomed footprint so the scrollbars/scroll range match what's on screen.
const MIN_ZOOM = 0.35;
const MAX_ZOOM = 2.5;
let canvasZoom = 1;

function renderCanvas(s) {
    const inner = document.getElementById('canvas-inner');
    const sizer = document.getElementById('canvas-sizer');
    const scrollEl = document.getElementById('canvas-scroll');
    const strands = s.current_macro?.strands ?? [];
    const macroId = s.current_macro?.id ?? null;
    if (macroId !== currentMacroId) canvasZoom = 1;

    const savedFocus = saveFocusedInput();

    const invalidBuffers = s.invalid_field_buffers ?? [];
    const currentInvalidKeys = new Set(invalidBuffers.map(b => `${b.strand_id}:${b.instruction_index}:${b.field_id}`));

    // Each strand-card sizes itself to its own content (no universal strand
    // width), so bounds can't be estimated up front — build every card first
    // (unpositioned), measure its natural size, then place it. This all
    // happens in one synchronous pass before the browser paints.
    inner.replaceChildren();
    const cardEls = new Map();
    for (const strand of strands) {
        if (drag && drag.resolvedId === strand.id) continue; // being dragged — the ghost stands in for it
        const card = buildStrandCard(strand, s, invalidBuffers, currentInvalidKeys);
        inner.appendChild(card);
        cardEls.set(strand.id, card);
    }

    // Bounding box (always covering at least ROOT_MARGIN around the origin —
    // where the root strand defaults to — so there's always generous room to
    // pan around it in every direction, even on a mostly-empty macro) so the
    // canvas can be scrolled out to whichever stray strand is farthest away.
    const prevMinX = lastBounds.minX, prevMinY = lastBounds.minY;
    let minX = -ROOT_MARGIN, minY = -ROOT_MARGIN, maxX = ROOT_MARGIN, maxY = ROOT_MARGIN;
    for (const strand of strands) {
        const card = cardEls.get(strand.id);
        if (!card) continue;
        minX = Math.min(minX, strand.x);
        minY = Math.min(minY, strand.y);
        maxX = Math.max(maxX, strand.x + card.offsetWidth);
        maxY = Math.max(maxY, strand.y + card.offsetHeight);
    }
    lastBounds = { minX, minY };

    const innerW = maxX - minX + 2 * CANVAS_PAD;
    const innerH = maxY - minY + 2 * CANVAS_PAD;
    inner.style.width = innerW + 'px';
    inner.style.height = innerH + 'px';
    inner.style.transform = `scale(${canvasZoom})`;
    sizer.style.width = (innerW * canvasZoom) + 'px';
    sizer.style.height = (innerH * canvasZoom) + 'px';

    // A strand placed beyond the previous extent (e.g. a palette drop out in
    // blank space) pushes minX/minY outward, which shifts every card's
    // rendered position by the same amount (see the loop below). Without
    // this, that shift happens under a scroll position that didn't move,
    // so the thing that was just dropped visibly jumps away from the
    // cursor — compensate scroll so the content the user was looking at
    // stays exactly where it was.
    if (minX !== prevMinX || minY !== prevMinY) {
        scrollEl.scrollLeft += (prevMinX - minX) * canvasZoom;
        scrollEl.scrollTop += (prevMinY - minY) * canvasZoom;
    }

    for (const strand of strands) {
        const card = cardEls.get(strand.id);
        if (!card) continue;
        card.style.left = (strand.x - minX + CANVAS_PAD) + 'px';
        card.style.top = (strand.y - minY + CANVAS_PAD) + 'px';
    }

    restoreFocusedInput(savedFocus);
    prevInvalidKeys = currentInvalidKeys;

    if (macroId !== currentMacroId) {
        currentMacroId = macroId;
        const root = strands.find(st => st.id === ROOT_ID);
        requestAnimationFrame(() => {
            if (root) {
                scrollEl.scrollLeft = Math.max(0, root.x - minX + CANVAS_PAD - 60);
                scrollEl.scrollTop = Math.max(0, root.y - minY + CANVAS_PAD - 60);
            } else {
                scrollEl.scrollLeft = 0;
                scrollEl.scrollTop = 0;
            }
        });
    }
}

// A strand is just its blocks: `.strand-card` is a bare positioning
// container with no visuals of its own — every instruction is its own
// bordered box, stacked vertically, each sized to only its own content. The
// only strand-level chrome is the root strand's small "Root" marker block,
// which sits above its stack and is the one thing that gets an accent
// outline (nothing else about the strand is outlined).
function buildStrandCard(strand, s, invalidBuffers, currentInvalidKeys) {
    const card = document.createElement('div');
    card.className = 'strand-card' + (strand.id === ROOT_ID ? ' is-root' : '');
    card.dataset.strandId = strand.id;

    const body = document.createElement('div');
    body.className = 'strand-body';

    if (strand.id === ROOT_ID) {
        body.appendChild(buildRootMarker());
    }

    if (strand.instructions.length === 0) {
        const hint = document.createElement('div');
        hint.className = 'strand-empty-hint';
        hint.textContent = 'Empty — drag an instruction here from the sidebar.';
        if (strand.id !== ROOT_ID) {
            hint.addEventListener('pointerdown', e => beginPickup(e, strand.id, 0));
        }
        body.appendChild(hint);
    } else {
        strand.instructions.forEach((ins, i) => {
            body.appendChild(buildInstructionRow(strand.id, i, ins, s.key_capture, invalidBuffers, prevInvalidKeys));
        });
    }
    card.appendChild(body);

    return card;
}

function buildRootMarker() {
    const marker = document.createElement('div');
    marker.className = 'root-marker';
    marker.appendChild(iconEl('play'));
    const label = document.createElement('span');
    label.textContent = 'Root';
    marker.appendChild(label);
    return marker;
}

function findStrand(strandId) {
    return state.current_macro?.strands?.find(st => st.id === strandId);
}

function getInvalidText(invalidBuffers, prevInvalidKeys, strandId, idx, fieldId) {
    const entry = invalidBuffers?.find(b => b.strand_id === strandId && b.instruction_index === idx && b.field_id === fieldId);
    if (!entry) return null;
    const isNew = !prevInvalidKeys.has(`${strandId}:${idx}:${fieldId}`);
    const trimmed = entry.text.trim();
    let invalid = true;
    if (trimmed !== '') {
        const num = Number(trimmed);
        if (!isNaN(num)) {
            if (fieldId === 'WaitDuration' || fieldId === 'WaitRandomness') {
                invalid = false;
            } else {
                invalid = !Number.isInteger(num);
            }
        }
    }
    return { text: entry.text, invalid, isNew };
}

function buildInstructionRow(strandId, i, ins, keyCapture, invalidBuffers, prevInvalidKeys) {
    const row = document.createElement('div');
    row.className = 'instruction-row';
    row.dataset.index = String(i);

    const grip = document.createElement('span');
    grip.className = 'row-grip';
    grip.appendChild(iconEl('move'));
    grip.title = 'Drag to move or detach';
    grip.addEventListener('pointerdown', e => beginPickup(e, strandId, i));
    row.appendChild(grip);

    const content = document.createElement('div');
    content.className = 'instruction-content';
    buildInstructionContent(content, strandId, i, ins, keyCapture, invalidBuffers, prevInvalidKeys);
    row.appendChild(content);

    // Reordering/removal/inserting now all happen by dragging blocks (the
    // grip above, or a fresh block from the sidebar palette) rather than
    // per-row buttons.
    return row;
}

function buildInstructionContent(content, strandId, i, ins, keyCapture, invalidBuffers, prevInvalidKeys) {
    const label = document.createElement('span');
    label.className = 'instruction-label';

    switch (ins.type) {
        case 'Wait': {
            label.textContent = 'Wait (ms):';
            const durBuf = getInvalidText(invalidBuffers, prevInvalidKeys, strandId, i, 'WaitDuration');
            const randBuf = getInvalidText(invalidBuffers, prevInvalidKeys, strandId, i, 'WaitRandomness');
            const durInput = numInput(durBuf?.text ?? String(ins.duration), durBuf?.invalid, durBuf?.isNew, v =>
                invoke('edit_instruction_field', { strandId, index: i, fieldId: 'WaitDuration', text: v }), strandId, i, 'WaitDuration');
            const randInput = numInput(randBuf?.text ?? String(ins.randomness), randBuf?.invalid, randBuf?.isNew, v =>
                invoke('edit_instruction_field', { strandId, index: i, fieldId: 'WaitRandomness', text: v }), strandId, i, 'WaitRandomness');
            const randLabel = document.createElement('span');
            randLabel.className = 'instruction-label';
            randLabel.textContent = '± random:';
            content.appendChild(label);
            content.appendChild(durInput);
            content.appendChild(randLabel);
            content.appendChild(randInput);
            break;
        }
        case 'Text': {
            label.textContent = 'Text:';
            const inp = textInput(ins.text, v =>
                invoke('edit_instruction', { strandId, index: i, instruction: { type: 'Text', text: v } }), strandId, i, 'Text');
            content.appendChild(label);
            content.appendChild(inp);
            break;
        }
        case 'Key': {
            label.textContent = 'Key:';
            const isCapturing = keyCapture?.strand_id === strandId && keyCapture?.index === i;
            const captureBtn = document.createElement('button');
            captureBtn.className = 'btn-chip key-capture-btn' + (isCapturing ? ' capturing' : '');
            captureBtn.textContent = isCapturing ? 'Press any key…' : ins.key;
            captureBtn.onclick = () => invoke('start_key_capture', { strandId, index: i });

            const dirSel = directionSelect(ins.direction, dir =>
                invoke('edit_instruction', { strandId, index: i, instruction: { type: 'Key', key: ins.key, direction: dir } }));
            content.appendChild(label);
            content.appendChild(captureBtn);
            content.appendChild(dirSel);
            break;
        }
        case 'Button': {
            label.textContent = 'Mouse:';
            const buttons = ['Left', 'Right', 'Middle', 'Side', 'Extra'];
            const btnSel = enumSelect(buttons, ins.button, v =>
                invoke('edit_instruction', { strandId, index: i, instruction: { type: 'Button', button: v, direction: ins.direction } }));
            const dirSel = directionSelect(ins.direction, dir =>
                invoke('edit_instruction', { strandId, index: i, instruction: { type: 'Button', button: ins.button, direction: dir } }));
            content.appendChild(label);
            content.appendChild(btnSel);
            content.appendChild(dirSel);
            break;
        }
        case 'MoveMouse': {
            label.textContent = 'Move mouse:';
            const xBuf = getInvalidText(invalidBuffers, prevInvalidKeys, strandId, i, 'MoveMouseX');
            const yBuf = getInvalidText(invalidBuffers, prevInvalidKeys, strandId, i, 'MoveMouseY');
            const xInput = numInput(xBuf?.text ?? String(ins.x), xBuf?.invalid, xBuf?.isNew, v =>
                invoke('edit_instruction_field', { strandId, index: i, fieldId: 'MoveMouseX', text: v }), strandId, i, 'MoveMouseX');
            const yInput = numInput(yBuf?.text ?? String(ins.y), yBuf?.invalid, yBuf?.isNew, v =>
                invoke('edit_instruction_field', { strandId, index: i, fieldId: 'MoveMouseY', text: v }), strandId, i, 'MoveMouseY');
            xInput.placeholder = 'X';
            yInput.placeholder = 'Y';
            const coordSel = enumSelect(['Absolute', 'Relative'], ins.coordinate, v =>
                invoke('edit_instruction', { strandId, index: i, instruction: { type: 'MoveMouse', x: ins.x, y: ins.y, coordinate: v } }));
            content.appendChild(label);
            content.appendChild(xInput);
            content.appendChild(yInput);
            content.appendChild(coordSel);
            break;
        }
        case 'Scroll': {
            label.textContent = 'Scroll:';
            const amtBuf = getInvalidText(invalidBuffers, prevInvalidKeys, strandId, i, 'ScrollAmount');
            const amtInput = numInput(amtBuf?.text ?? String(ins.amount), amtBuf?.invalid, amtBuf?.isNew, v =>
                invoke('edit_instruction_field', { strandId, index: i, fieldId: 'ScrollAmount', text: v }), strandId, i, 'ScrollAmount');
            const axisSel = enumSelect(['Vertical', 'Horizontal'], ins.axis, v =>
                invoke('edit_instruction', { strandId, index: i, instruction: { type: 'Scroll', amount: ins.amount, axis: v } }));
            content.appendChild(label);
            content.appendChild(amtInput);
            content.appendChild(axisSel);
            break;
        }
        case 'Command': {
            label.textContent = 'Command:';
            const inp = textInput(ins.command, v =>
                invoke('edit_instruction', { strandId, index: i, instruction: { type: 'Command', command: v } }), strandId, i, 'Command');
            inp.placeholder = 'bash -c …';
            content.appendChild(label);
            content.appendChild(inp);
            break;
        }
        case 'Comment': {
            label.textContent = '//';
            const inp = textInput(ins.comment, v =>
                invoke('edit_instruction', { strandId, index: i, instruction: { type: 'Comment', comment: v } }), strandId, i, 'Comment');
            inp.placeholder = 'Comment';
            inp.style.fontStyle = 'italic';
            inp.style.color = 'var(--text-dim)';
            content.appendChild(label);
            content.appendChild(inp);
            break;
        }
        default: {
            label.textContent = ins.type;
            content.appendChild(label);
        }
    }
}

// ─── Widget helpers ───────────────────────────────────────────────────────────

// Sizes an <input> to its own content in characters (via the `size`
// attribute) instead of a fixed pixel width, so each block is only as wide
// as what's actually typed into it — starting short and growing as the
// user types more.
function autosizeInput(inp, minChars) {
    const resize = () => { inp.size = Math.max(minChars, inp.value.length); };
    resize();
    inp.addEventListener('input', resize);
}

function textInput(value, onChange, strandId, ix, field) {
    const inp = document.createElement('input');
    inp.type = 'text';
    inp.value = value;
    autosizeInput(inp, 6);
    if (strandId != null && ix != null && field != null) { inp.dataset.strand = strandId; inp.dataset.ix = String(ix); inp.dataset.field = field; }
    inp.addEventListener('input', () => onChange(inp.value));
    return inp;
}

function numInput(value, invalid, isNew, onChange, strandId, ix, field) {
    const inp = document.createElement('input');
    inp.type = 'text';
    inp.value = value;
    autosizeInput(inp, 3);
    if (invalid) {
        inp.classList.add('invalid');
        if (isNew) inp.classList.add('shake-once');
    }
    if (strandId != null && ix != null && field != null) { inp.dataset.strand = strandId; inp.dataset.ix = String(ix); inp.dataset.field = field; }
    inp.addEventListener('input', () => onChange(inp.value));
    return inp;
}

function directionSelect(current, onChange) {
    return enumSelect(['Click', 'Press', 'Release'], current, onChange);
}

function enumSelect(options, current, onChange) {
    return dropdown(options, current, onChange, { className: 'dd-compact' });
}

// ─── Add instruction at index ─────────────────────────────────────────────────

function defaultInstruction(type) {
    switch (type) {
        case 'Wait':      return { type: 'Wait', duration: 1000, randomness: 0 };
        case 'Text':      return { type: 'Text', text: 'text' };
        case 'Key':       return { type: 'Key', key: 'a', direction: 'Click' };
        case 'Button':    return { type: 'Button', button: 'Left', direction: 'Click' };
        case 'MoveMouse': return { type: 'MoveMouse', x: 0, y: 0, coordinate: 'Relative' };
        case 'Scroll':    return { type: 'Scroll', amount: 4, axis: 'Vertical' };
        case 'Command':   return { type: 'Command', command: '' };
        case 'Comment':   return { type: 'Comment', comment: '' };
        default:          return { type: 'Comment', comment: '' };
    }
}

async function addInstructionAt(strandId, index, type) {
    const ins = defaultInstruction(type);
    try {
        await invoke('add_instruction', { strandId, index, instruction: ins });
    } catch (err) {
        console.error('add_instruction failed:', err);
    }
}

// ═══ Drag & drop (pick up a block, snap onto another strand, or drop free) ═══

const SNAP_THRESHOLD = 28;
let dragCandidate = null; // pointerdown seen, waiting to see if it turns into a drag
let drag = null;          // an active drag, once the pointer has moved past the threshold

// Pointer capture ensures this pointer's move/up events keep reaching us
// even if the cursor leaves the window mid-drag — without it a release
// outside the webview can be missed entirely, leaving `pan`/`drag`/
// `paletteDrag` stuck set and silently blocking every later gesture.
function capturePointer(e) {
    try {
        e.target.setPointerCapture?.(e.pointerId);
    } catch (err) {
        console.error('setPointerCapture failed:', err);
    }
}

function beginPickup(e, strandId, index) {
    if (state.recording_phase?.phase === 'Active') return;
    if (e.button !== undefined && e.button !== 0) return;
    e.preventDefault();
    capturePointer(e);
    dragCandidate = { strandId, index, pointerId: e.pointerId, startX: e.clientX, startY: e.clientY };
}

// ── Middle-click-drag panning ───────────────────────────────────────────────
let pan = null;

function beginPan(e) {
    if (e.button !== 1) return;
    e.preventDefault();
    capturePointer(e);
    const scrollEl = document.getElementById('canvas-scroll');
    pan = {
        pointerId: e.pointerId,
        startX: e.clientX,
        startY: e.clientY,
        startScrollLeft: scrollEl.scrollLeft,
        startScrollTop: scrollEl.scrollTop,
    };
    scrollEl.classList.add('panning');
}

function attachDragListeners() {
    document.getElementById('canvas-scroll').addEventListener('pointerdown', beginPan);
    document.addEventListener('pointermove', onPointerMove);
    document.addEventListener('pointerup', onPointerUp);
    document.addEventListener('pointercancel', onPointerUp);
    document.getElementById('canvas-scroll').addEventListener('wheel', onCanvasWheel, { passive: false });
}

function onCanvasWheel(e) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    const scrollEl = document.getElementById('canvas-scroll');
    const rect = scrollEl.getBoundingClientRect();
    // Point under the cursor, in unscaled canvas-space px, kept stable across the zoom change.
    const canvasPtX = (e.clientX - rect.left + scrollEl.scrollLeft) / canvasZoom;
    const canvasPtY = (e.clientY - rect.top + scrollEl.scrollTop) / canvasZoom;

    const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12; // scroll up = zoom in, scroll down = zoom out
    const newZoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, canvasZoom * factor));
    if (newZoom === canvasZoom) return;
    canvasZoom = newZoom;
    renderCanvas(state);

    scrollEl.scrollLeft = canvasPtX * canvasZoom - (e.clientX - rect.left);
    scrollEl.scrollTop = canvasPtY * canvasZoom - (e.clientY - rect.top);
}

// ── Instruction sidebar (palette + trash) ───────────────────────────────────

function buildSidebarPalette() {
    const palette = document.getElementById('sidebar-palette');
    palette.replaceChildren();
    for (const type of Object.keys(INSTRUCTION_TYPE_LABELS)) {
        const block = document.createElement('div');
        block.className = 'palette-block';
        block.appendChild(iconEl(INSTRUCTION_TYPE_ICONS[type]));
        const label = document.createElement('span');
        label.textContent = INSTRUCTION_TYPE_LABELS[type];
        block.appendChild(label);
        block.addEventListener('pointerdown', e => beginPaletteDrag(e, type));
        palette.appendChild(block);
    }
}

function isOverSidebar(e) {
    const rect = document.getElementById('instruction-sidebar').getBoundingClientRect();
    return e.clientX >= rect.left && e.clientX <= rect.right && e.clientY >= rect.top && e.clientY <= rect.bottom;
}

function setSidebarArmed(armed) {
    document.getElementById('instruction-sidebar').classList.toggle('trash-armed', armed);
}

// Dragging a palette entry previews the actual instruction block it'll
// create (not the sidebar chip) — dropped on empty canvas it becomes exactly
// that: a single ordinary-looking block, nothing else.
function beginPaletteDrag(e, insType) {
    if (state.recording_phase?.phase === 'Active') return;
    if (e.button !== undefined && e.button !== 0) return;
    e.preventDefault();
    capturePointer(e);

    const ghost = document.createElement('div');
    ghost.className = 'strand-drag-ghost';
    const ghostCard = document.createElement('div');
    ghostCard.className = 'strand-card';
    const ghostBody = document.createElement('div');
    ghostBody.className = 'strand-body';
    ghostBody.appendChild(buildInstructionRow('__palette__', 0, defaultInstruction(insType), null, [], new Set()));
    ghostCard.appendChild(ghostBody);
    ghost.appendChild(ghostCard);
    document.body.appendChild(ghost);

    paletteDrag = {
        pointerId: e.pointerId,
        insType,
        offsetX: 14,
        offsetY: 14,
        ghostEl: ghost,
        snap: null,
    };
    positionGhost(e);
}

// ═══ Drag & drop (pick up a block, snap onto another strand, or drop free) ═══

let paletteDrag = null; // dragging a fresh instruction in from the sidebar palette

// Each branch below only ever `return`s once it has confirmed the event's
// pointerId actually belongs to that gesture — never on a bare "some other
// gesture is active" check. Only one of pan/paletteDrag/drag/dragCandidate
// is ever really in flight at once, but if any one of them were ever left
// stuck (e.g. a pointerup missed while panning), a blanket early-return here
// would silently swallow every *other* pointer's moves/ups forever, which is
// exactly what made palette drops stop working.
function onPointerMove(e) {
    if (pan && e.pointerId === pan.pointerId) {
        const scrollEl = document.getElementById('canvas-scroll');
        scrollEl.scrollLeft = pan.startScrollLeft - (e.clientX - pan.startX);
        scrollEl.scrollTop = pan.startScrollTop - (e.clientY - pan.startY);
        return;
    }
    if (paletteDrag && e.pointerId === paletteDrag.pointerId) {
        positionGhost(e);
        if (isOverSidebar(e)) {
            paletteDrag.snap = null;
            clearSnapIndicator();
        } else {
            updateSnapTarget(e, paletteDrag, null);
        }
        return;
    }
    if (drag && e.pointerId === drag.pointerId) {
        positionGhost(e);
        if (isOverSidebar(e)) {
            drag.overTrash = true;
            drag.snap = null;
            clearSnapIndicator();
            setSidebarArmed(true);
        } else {
            drag.overTrash = false;
            setSidebarArmed(false);
            updateSnapTarget(e, drag, drag.resolvedId ?? dragCandidate?.strandId);
        }
        return;
    }
    if (dragCandidate && e.pointerId === dragCandidate.pointerId) {
        const dx = e.clientX - dragCandidate.startX;
        const dy = e.clientY - dragCandidate.startY;
        if (Math.hypot(dx, dy) < 4) return;
        startDrag(e, dragCandidate);
        dragCandidate = null;
    }
}

// The ghost is built from real `.strand-card`/`.instruction-row` clones (not
// a specially-styled wrapper), so a block being dragged looks exactly like
// it does at rest — the block is the strand, dragging just moves it.
function startDrag(e, candidate) {
    const { strandId, index, pointerId } = candidate;
    const strand = findStrand(strandId);
    if (!strand) return;

    const cardEl = document.querySelector(`.strand-card[data-strand-id="${cssEscape(strandId)}"]`);
    const wholeStrandGrab = strandId !== ROOT_ID && index === 0;

    const ghost = document.createElement('div');
    ghost.className = 'strand-drag-ghost';
    let anchorRect = cardEl ? cardEl.getBoundingClientRect() : { left: e.clientX, top: e.clientY };

    if (wholeStrandGrab) {
        if (cardEl) {
            ghost.appendChild(cardEl.cloneNode(true));
            cardEl.style.visibility = 'hidden';
        }
    } else {
        const rowEls = cardEl ? Array.from(cardEl.querySelectorAll('.instruction-row')).slice(index) : [];
        if (rowEls[0]) anchorRect = rowEls[0].getBoundingClientRect();
        const ghostCard = document.createElement('div');
        ghostCard.className = 'strand-card';
        const ghostBody = document.createElement('div');
        ghostBody.className = 'strand-body';
        rowEls.forEach(el => {
            el.style.visibility = 'hidden';
            ghostBody.appendChild(el.cloneNode(true));
        });
        ghostCard.appendChild(ghostBody);
        ghost.appendChild(ghostCard);
    }
    document.body.appendChild(ghost);

    drag = {
        pointerId,
        offsetX: e.clientX - anchorRect.left,
        offsetY: e.clientY - anchorRect.top,
        ghostEl: ghost,
        resolvedId: null,
        resolvingPromise: null,
        snap: null,
    };

    if (wholeStrandGrab) {
        drag.resolvedId = strandId;
    } else {
        const localDrag = drag;
        drag.resolvingPromise = invoke('split_strand', { strandId, index, x: strand.x + 24, y: strand.y + 24 })
            .then(newId => { localDrag.resolvedId = newId; return newId; })
            .catch(err => { console.error('split_strand failed:', err); });
    }

    positionGhost(e);
}

let ghostRafPending = false;
let lastPointerEvent = null;
function positionGhost(e) {
    lastPointerEvent = e;
    if (ghostRafPending) return;
    ghostRafPending = true;
    requestAnimationFrame(() => {
        ghostRafPending = false;
        const active = drag || paletteDrag;
        if (!active || !lastPointerEvent) return;
        active.ghostEl.style.transform = `translate(${lastPointerEvent.clientX - active.offsetX}px, ${lastPointerEvent.clientY - active.offsetY}px)`;
    });
}

let snapIndicatorEl = null;
function clearSnapIndicator() {
    if (snapIndicatorEl) { snapIndicatorEl.remove(); snapIndicatorEl = null; }
}

// Shared by both strand-drags (snapping an existing block elsewhere) and
// palette-drags (dropping a brand new instruction onto a strand); writes the
// result onto `target.snap` and updates the shared snap-line indicator.
function updateSnapTarget(e, target, excludeId) {
    const cards = Array.from(document.querySelectorAll('.strand-card'));
    let best = null;
    for (const card of cards) {
        const id = card.dataset.strandId;
        if (id === excludeId) continue;
        const cardRect = card.getBoundingClientRect();
        if (e.clientX < cardRect.left - 60 || e.clientX > cardRect.right + 60) continue;
        const body = card.querySelector('.strand-body');
        const rows = Array.from(card.querySelectorAll('.instruction-row'));
        const boundaries = rows.map(r => r.getBoundingClientRect().top);
        boundaries.push(rows.length ? rows[rows.length - 1].getBoundingClientRect().bottom : body.getBoundingClientRect().top + 8);
        boundaries.forEach((y, idx) => {
            const dist = Math.abs(e.clientY - y);
            if (dist <= SNAP_THRESHOLD && (!best || dist < best.dist)) {
                best = { targetId: id, index: idx, dist, y, left: cardRect.left, width: cardRect.width };
            }
        });
    }
    target.snap = best ? { targetId: best.targetId, index: best.index } : null;

    if (best) {
        if (!snapIndicatorEl) {
            snapIndicatorEl = document.createElement('div');
            snapIndicatorEl.className = 'strand-snap-indicator';
            document.body.appendChild(snapIndicatorEl);
        }
        snapIndicatorEl.style.left = best.left + 'px';
        snapIndicatorEl.style.top = (best.y - 2) + 'px';
        snapIndicatorEl.style.width = best.width + 'px';
    } else {
        clearSnapIndicator();
    }
}

function clientToCanvas(clientX, clientY) {
    const inner = document.getElementById('canvas-inner');
    const rect = inner.getBoundingClientRect(); // reflects the current zoom transform
    return [
        Math.round((clientX - rect.left) / canvasZoom - CANVAS_PAD + lastBounds.minX),
        Math.round((clientY - rect.top) / canvasZoom - CANVAS_PAD + lastBounds.minY),
    ];
}

function onPointerUp(e) {
    if (pan && pan.pointerId === e.pointerId) {
        pan = null;
        document.getElementById('canvas-scroll').classList.remove('panning');
    }
    if (dragCandidate && dragCandidate.pointerId === e.pointerId) {
        dragCandidate = null;
    }

    if (paletteDrag && paletteDrag.pointerId === e.pointerId) {
        const finished = paletteDrag;
        paletteDrag = null;
        clearSnapIndicator();
        finished.ghostEl.remove();

        if (!isOverSidebar(e)) {
            const ins = defaultInstruction(finished.insType);
            (async () => {
                try {
                    if (finished.snap) {
                        await invoke('add_instruction', { strandId: finished.snap.targetId, index: finished.snap.index, instruction: ins });
                    } else {
                        const [x, y] = clientToCanvas(e.clientX - finished.offsetX, e.clientY - finished.offsetY);
                        await invoke('add_strand', { x, y, instruction: ins });
                    }
                } catch (err) {
                    console.error('palette drop failed:', err);
                }
            })();
        }
        return;
    }

    if (!drag || drag.pointerId !== e.pointerId) return;

    const finished = drag;
    drag = null;
    clearSnapIndicator();
    setSidebarArmed(false);
    finished.ghostEl.remove();

    (async () => {
        const id = finished.resolvedId ?? (finished.resolvingPromise ? await finished.resolvingPromise : null);
        if (!id) { render(state); return; }
        if (finished.overTrash) {
            await invoke('remove_strand', { strandId: id });
        } else if (finished.snap) {
            await invoke('merge_strand', { draggedId: id, targetId: finished.snap.targetId, index: finished.snap.index });
        } else {
            const [x, y] = clientToCanvas(e.clientX - finished.offsetX, e.clientY - finished.offsetY);
            await invoke('move_strand', { strandId: id, x, y });
        }
    })();
}

// ═══ Settings Page ════════════════════════════════════════════════════════════

function renderSettings(s) {
    renderWarnings(s);
    renderGlobalHotkeys(s);
    renderPerMacroHotkeys(s);
    renderTcpServer(s);
    renderUpdates(s);
}

function renderWarnings(s) {
    const container = document.getElementById('warnings-container');
    container.innerHTML = '';
    const grabMissing = !s.grab_available;
    const emulatorMissing = !s.emulator_available;
    if (grabMissing) {
        container.appendChild(buildWarningBanner(prevWarnings.grab,
            'Global hotkeys unavailable.<br>Check system permissions (Accessibility / input group).'));
    }
    if (emulatorMissing) {
        container.appendChild(buildWarningBanner(prevWarnings.emulator,
            'Input emulation unavailable.<br>Check system permissions.'));
    }
    prevWarnings = { grab: grabMissing, emulator: emulatorMissing };
}

function buildWarningBanner(alreadyShown, html) {
    const banner = document.createElement('div');
    banner.className = 'warning-banner' + (alreadyShown ? '' : ' banner-enter');
    banner.appendChild(iconEl('alert-triangle'));
    const text = document.createElement('span');
    text.innerHTML = html;
    banner.appendChild(text);
    return banner;
}

const NAMED_ACTIONS = [
    { label: 'Run Macro',                    type: 'RunMacro' },
    { label: 'Stop Loop',                    type: 'StopLoop' },
    { label: 'Next Macro',                   type: 'NextMacro' },
    { label: 'Previous Macro',               type: 'PrevMacro' },
    { label: 'Toggle Loop',                  type: 'ToggleLoop' },
    { label: 'Start Recording (immediate)',  type: 'StartRecordingImmediate' },
];

function renderGlobalHotkeys(s) {
    const list = document.getElementById('global-hotkeys-list');
    list.innerHTML = '';

    NAMED_ACTIONS.forEach(({ label, type }) => {
        const binding = (s.hotkey_bindings ?? []).find(b => b.action.type === type);
        const comboDisplay = binding?.combo_display ?? null;
        const isCapturing = s.combo_capture?.kind === 'Named' && s.combo_capture?.action?.type === type;

        const row = document.createElement('div');
        row.className = 'settings-row';

        const labelEl = document.createElement('span');
        labelEl.className = 'settings-row-label';
        labelEl.textContent = label;

        const comboBtn = document.createElement('button');
        comboBtn.className = 'btn-chip' + (isCapturing ? ' capturing' : '');
        if (isCapturing) {
            comboBtn.textContent = 'Press combo…';
            comboBtn.disabled = false;
        } else {
            comboBtn.textContent = comboDisplay ?? 'Not set';
            comboBtn.onclick = () => invoke('start_combo_capture', { action: { type } });
        }

        // Default button
        const defBtn = document.createElement('button');
        defBtn.textContent = 'Default';
        defBtn.style.display = (isCapturing || comboDisplay == null) ? 'none' : '';
        defBtn.onclick = () => invoke('reset_hotkey_to_default', { action: { type } });

        // Clear button
        const clearBtn = document.createElement('button');
        clearBtn.className = 'btn-icon btn-danger';
        clearBtn.appendChild(iconEl('x'));
        clearBtn.title = 'Clear hotkey';
        clearBtn.setAttribute('aria-label', 'Clear hotkey');
        clearBtn.style.display = (isCapturing || comboDisplay == null) ? 'none' : '';
        clearBtn.onclick = () => invoke('clear_named_hotkey', { action: { type } });

        row.appendChild(labelEl);
        row.appendChild(comboBtn);
        row.appendChild(defBtn);
        row.appendChild(clearBtn);
        list.appendChild(row);
    });
}

function renderPerMacroHotkeys(s) {
    const list = document.getElementById('per-macro-hotkeys-list');
    list.innerHTML = '';

    const perMacroBindings = (s.hotkey_bindings ?? []).filter(b => b.action.type === 'RunSpecificMacro');
    perMacroBindings.forEach(b => {
        const row = document.createElement('div');
        row.className = 'settings-row';

        const name = document.createElement('span');
        name.className = 'settings-row-label';
        name.textContent = b.macro_name ?? '(deleted)';

        const comboBtn = document.createElement('button');
        comboBtn.className = 'btn-chip';
        comboBtn.textContent = b.combo_display;
        // Re-capture not wired for per-macro (matches existing app behaviour)

        const removeBtn = document.createElement('button');
        removeBtn.className = 'btn-icon btn-danger';
        removeBtn.appendChild(iconEl('x'));
        removeBtn.title = 'Remove hotkey';
        removeBtn.setAttribute('aria-label', 'Remove hotkey');
        removeBtn.onclick = () => invoke('remove_hotkey_binding', { index: b.binding_index });

        row.appendChild(name);
        row.appendChild(comboBtn);
        row.appendChild(removeBtn);
        list.appendChild(row);
    });

    // Update Add form
    pendingMacroDropdown.ddSetOptions((s.macro_names ?? []).map((name, idx) => ({ value: String(idx), label: name })));

    const isCapturingPending = s.combo_capture?.kind === 'Pending';
    const pendingCombo = s.pending_macro_hotkey?.combo_display;
    const pendingComboBtn = document.getElementById('pending-combo-btn');
    pendingComboBtn.classList.toggle('capturing', isCapturingPending);
    if (isCapturingPending) {
        pendingComboBtn.textContent = 'Press combo…';
    } else {
        pendingComboBtn.textContent = pendingCombo ?? 'Set combo';
        pendingComboBtn.onclick = () => invoke('start_pending_combo_capture');
    }

    const addBtn = document.getElementById('add-macro-hotkey-btn');
    const canAdd = s.pending_macro_hotkey?.macro_index != null && pendingCombo != null;
    addBtn.disabled = !canAdd;
}

function renderTcpServer(s) {
    const section = document.getElementById('tcp-server-section');
    section.innerHTML = '';

    // Port row
    const portRow = document.createElement('div');
    portRow.className = 'settings-row';
    const portLabel = document.createElement('span');
    portLabel.className = 'settings-row-label';
    portLabel.textContent = 'Port';
    const portInput = document.createElement('input');
    portInput.type = 'text';
    portInput.value = s.ipc_port_text;
    portInput.style.width = '80px';
    if (s.ipc_port_invalid) {
        portInput.classList.add('invalid');
        if (!prevPortInvalid) portInput.classList.add('shake-once');
    }
    portInput.addEventListener('input', () => invoke('set_ipc_port_text', { text: portInput.value }));
    portRow.appendChild(portLabel);
    portRow.appendChild(portInput);
    section.appendChild(portRow);
    prevPortInvalid = s.ipc_port_invalid;

    // Status + toggle row
    const statusRow = document.createElement('div');
    statusRow.className = 'settings-row';
    const statusLabel = document.createElement('span');
    statusLabel.className = 'settings-row-label';
    statusLabel.textContent = s.ipc_active_port != null
        ? `Listening on 127.0.0.1:${s.ipc_active_port}`
        : 'Stopped';
    const toggleBtn = document.createElement('button');
    if (s.ipc_active_port != null) {
        toggleBtn.textContent = 'Stop Server';
        toggleBtn.onclick = () => invoke('stop_ipc_server');
    } else {
        toggleBtn.textContent = 'Start Server';
        toggleBtn.disabled = s.ipc_port_invalid;
        toggleBtn.onclick = () => invoke('start_ipc_server');
    }
    statusRow.appendChild(statusLabel);
    statusRow.appendChild(toggleBtn);
    section.appendChild(statusRow);

    // Auto-start row
    const autoRow = document.createElement('div');
    autoRow.className = 'settings-row';
    const autoLabel = document.createElement('span');
    autoLabel.className = 'settings-row-label';
    autoLabel.textContent = 'Automatically start server on app launch';
    const autoSwitch = document.createElement('label');
    autoSwitch.className = 'switch';
    const autoCheck = document.createElement('input');
    autoCheck.type = 'checkbox';
    autoCheck.checked = s.ipc_auto_start;
    autoCheck.onchange = () => invoke('set_ipc_auto_start', { enabled: autoCheck.checked });
    const autoTrack = document.createElement('span');
    autoTrack.className = 'switch-track';
    autoSwitch.appendChild(autoCheck);
    autoSwitch.appendChild(autoTrack);
    autoRow.appendChild(autoLabel);
    autoRow.appendChild(autoSwitch);
    section.appendChild(autoRow);
}

function renderUpdates(s) {
    const section = document.getElementById('updates-section');
    const content = document.getElementById('updates-content');
    content.innerHTML = '';

    const uc = s.update_check_state;

    const versionRow = document.createElement('div');
    versionRow.className = 'settings-row';
    const versionLabel = document.createElement('span');
    versionLabel.className = 'settings-row-label';
    versionLabel.textContent = appVersion ? `Current version: ${appVersion}` : 'Updates';
    const checkBtn = document.createElement('button');
    checkBtn.textContent = 'Check for Updates';
    const busy = uc.state === 'Checking' || uc.state === 'Applying';
    checkBtn.disabled = busy;
    checkBtn.onclick = () => invoke('check_for_updates');
    versionRow.appendChild(versionLabel);
    versionRow.appendChild(checkBtn);
    content.appendChild(versionRow);

    if (uc.state !== 'Idle') {
        const statusRow = document.createElement('div');
        statusRow.className = 'settings-row';
        let msg = '';
        if (uc.state === 'Checking') msg = 'Checking for updates…';
        else if (uc.state === 'UpToDate') msg = 'Up to date';
        else if (uc.state === 'UpdateAvailable') msg = `Update available: ${uc.version}`;
        else if (uc.state === 'Applying') msg = 'Installing update…';
        else if (uc.state === 'Error') msg = `Update check failed: ${uc.error}`;
        statusRow.textContent = msg;
        content.appendChild(statusRow);
    }

    if (uc.state === 'UpdateAvailable') {
        const updateRow = document.createElement('div');
        updateRow.className = 'settings-row';
        const updateBtn = document.createElement('button');
        updateBtn.textContent = 'Update Now';
        updateBtn.onclick = () => invoke('apply_update');
        updateRow.appendChild(updateBtn);
        content.appendChild(updateRow);
    }
}

// ─── Static event listeners ───────────────────────────────────────────────────

function setupStaticListeners() {
    // Pointer-based drag & drop for the strand canvas
    attachDragListeners();
    buildSidebarPalette();

    // Macro selector
    macroDropdown = dropdown([], '', val => {
        if (val === '') return;
        const idx = parseInt(val);
        const cached = state.macros_data?.[idx];
        if (cached) {
            state.macro_selected = idx;
            state.current_macro = cached;
            state.can_undo = false;
            state.can_redo = false;
            state.invalid_field_buffers = [];
            state.key_capture = null;
            render(state);
        }
        invoke('select_macro', { index: idx });
    }, { placeholder: '— no macro selected —', ariaLabel: 'Select macro', className: 'macro-select-trigger' });
    macroDropdown.querySelector('.dd-trigger').setAttribute('aria-labelledby', 'macro-dropdown-label');
    document.getElementById('macro-dropdown-container').appendChild(macroDropdown);

    // Macro CRUD buttons
    document.getElementById('new-macro-btn').onclick = () => invoke('new_macro');
    document.getElementById('remove-macro-btn').onclick = () => invoke('remove_macro');
    document.getElementById('settings-btn').onclick = () => invoke('open_settings');

    // Run controls
    document.getElementById('run-macro-btn').onclick = () => invoke('run_macro');
    document.getElementById('loop-mode-check').onchange = e => {
        invoke('toggle_loop_mode', { enabled: e.target.checked });
    };
    document.getElementById('record-btn').onclick = () => {
        if (state.recording_phase?.phase === 'Idle') {
            invoke('start_recording');
        } else {
            invoke('stop_recording');
        }
    };

    // Macro title
    document.getElementById('macro-title').addEventListener('input', e => {
        invoke('set_title', { title: e.target.value });
    });

    // Editor toolbar
    document.getElementById('undo-btn').onclick = () => invoke('undo');
    document.getElementById('redo-btn').onclick = () => invoke('redo');
    document.getElementById('clear-instructions-btn').onclick = () => invoke('clear_instructions');
    document.getElementById('save-macro-btn').onclick = () => invoke('save_macro');

    // Settings back button
    document.getElementById('back-btn').onclick = () => invoke('close_settings');

    // Theme toggle
    document.getElementById('theme-toggle-btn').onclick = () => {
        setTheme(currentTheme === 'light' ? 'dark' : 'light');
    };
    updateThemeToggleIcon();

    // Per-macro hotkey add form
    pendingMacroDropdown = dropdown([], '', val => {
        invoke('set_pending_macro_idx', { index: val === '' ? null : parseInt(val) });
    }, { placeholder: 'Select macro…', ariaLabel: 'Select macro for hotkey' });
    document.getElementById('pending-macro-select-container').appendChild(pendingMacroDropdown);
    document.getElementById('add-macro-hotkey-btn').onclick = () => invoke('add_macro_hotkey');
}

// ─── Start ────────────────────────────────────────────────────────────────────

window.addEventListener('DOMContentLoaded', init);
