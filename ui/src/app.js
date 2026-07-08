import './style.css';
import { iconEl, setBtnContent, INSTRUCTION_TYPE_ICONS, INSTRUCTION_TYPE_LABELS } from './icons.js';
import { dropdown, closeAllDropdowns } from './dropdown.js';

// Tauri v2 API
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ─── Virtual scroll constants (mirror the Rust impl) ────────────────────────
const ROW_H = 60;
const BUFFER = 5;

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
let addInstructionType = 'Wait';
let addInstructionMainBtn = null;

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
    if (state.key_capture_index != null) {
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
        return;
    }
    editorEl.classList.remove('hidden');
    emptyStateEl.classList.add('hidden');

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

    renderInstructions(s);
}

function saveFocusedInput() {
    const el = document.activeElement;
    if (!el || el.dataset.ix === undefined || el.dataset.field === undefined) return null;
    return {
        ix: el.dataset.ix,
        field: el.dataset.field,
        start: el.selectionStart,
        end: el.selectionEnd,
    };
}

function restoreFocusedInput(saved) {
    if (!saved) return;
    const inner = document.getElementById('instructions-inner');
    if (!inner) return;
    const el = inner.querySelector(`[data-ix="${saved.ix}"][data-field="${saved.field}"]`);
    if (el) {
        el.focus();
        if (saved.start != null) {
            el.setSelectionRange(saved.start, saved.end);
        }
    }
}

function renderInstructions(s) {
    const scrollEl = document.getElementById('instructions-scroll');
    const inner = document.getElementById('instructions-inner');
    const instructions = s.current_macro?.instructions ?? [];
    const len = instructions.length;

    const savedFocus = saveFocusedInput();

    if (len === 0) {
        inner.style.paddingTop = '';
        inner.style.paddingBottom = '';
        if (!inner.querySelector('.empty-state')) {
            inner.replaceChildren(buildEmptyInstructionsState());
        }
        prevInvalidKeys = new Set();
        return;
    }

    const scrollTop = scrollEl.scrollTop;
    const viewportH = scrollEl.clientHeight || 600;

    const rawStart = Math.floor(scrollTop / ROW_H);
    const visibleCount = Math.ceil(viewportH / ROW_H) + 1;
    const startIdx = Math.max(0, rawStart - BUFFER);
    const endIdx = Math.min(len, rawStart + visibleCount + BUFFER);

    inner.style.paddingTop = (startIdx * ROW_H) + 'px';
    inner.style.paddingBottom = (Math.max(0, len - endIdx) * ROW_H) + 'px';

    const invalidBuffers = s.invalid_field_buffers ?? [];
    const currentInvalidKeys = new Set(invalidBuffers.map(b => `${b.instruction_index}:${b.field_id}`));

    // Map existing rows by data-index
    const existingRows = new Map();
    for (const child of inner.children) {
        const idx = parseInt(child.dataset.index);
        if (!isNaN(idx)) existingRows.set(idx, child);
    }

    // Remove rows that scrolled out of view
    for (const [idx, row] of existingRows) {
        if (idx < startIdx || idx >= endIdx) {
            row.remove();
            existingRows.delete(idx);
        }
    }

    // Update visible rows in place and insert new ones, maintaining DOM order
    let prevRow = null;
    for (let i = startIdx; i < endIdx; i++) {
        const existing = existingRows.get(i);
        if (existing) {
            updateInstructionRowContent(existing, i, instructions[i], s.key_capture_index, invalidBuffers, prevInvalidKeys);
            existingRows.delete(i);
            prevRow = existing;
        } else {
            const newRow = buildInstructionRow(i, instructions[i], s.key_capture_index, invalidBuffers, prevInvalidKeys);
            if (prevRow) {
                prevRow.after(newRow);
            } else {
                inner.prepend(newRow);
            }
            prevRow = newRow;
        }
    }

    restoreFocusedInput(savedFocus);

    prevInvalidKeys = currentInvalidKeys;
}

let scrollRafPending = false;
function attachScrollListener() {
    const scrollEl = document.getElementById('instructions-scroll');
    if (scrollEl) {
        scrollEl.addEventListener('scroll', () => {
            if (!scrollRafPending) {
                scrollRafPending = true;
                requestAnimationFrame(() => {
                    scrollRafPending = false;
                    if (state.current_macro) renderInstructions(state);
                });
            }
        });
    }
}

function getInvalidText(invalidBuffers, prevInvalidKeys, idx, fieldId) {
    const entry = invalidBuffers?.find(b => b.instruction_index === idx && b.field_id === fieldId);
    if (!entry) return null;
    const isNew = !prevInvalidKeys.has(`${idx}:${fieldId}`);
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

function buildEmptyInstructionsState() {
    const wrap = document.createElement('div');
    wrap.className = 'empty-state empty-state-inline';
    wrap.appendChild(iconEl('inbox'));
    const title = document.createElement('p');
    title.textContent = 'No instructions yet';
    const sub = document.createElement('span');
    sub.textContent = 'Add one below, or hit Record to capture actions live.';
    wrap.appendChild(title);
    wrap.appendChild(sub);
    return wrap;
}

function updateInstructionRowContent(row, i, ins, keyCaptureIdx, invalidBuffers, prevInvalidKeys) {
    const oldContent = row.querySelector('.instruction-content');
    const newContent = document.createElement('div');
    newContent.className = 'instruction-content';
    buildInstructionContent(newContent, i, ins, keyCaptureIdx, invalidBuffers, prevInvalidKeys);
    oldContent.replaceWith(newContent);
}

function buildInstructionRow(i, ins, keyCaptureIdx, invalidBuffers, prevInvalidKeys) {
    const row = document.createElement('div');
    row.className = 'instruction-row';
    row.dataset.index = String(i);

    const content = document.createElement('div');
    content.className = 'instruction-content';
    buildInstructionContent(content, i, ins, keyCaptureIdx, invalidBuffers, prevInvalidKeys);
    row.appendChild(content);

    // Controls: Up, Down, Remove, Add-after
    const controls = document.createElement('div');
    controls.className = 'row-controls';

    const upBtn = document.createElement('button');
    upBtn.className = 'btn-icon';
    upBtn.appendChild(iconEl('chevron-up'));
    upBtn.title = 'Move up';
    upBtn.setAttribute('aria-label', 'Move up');
    upBtn.onclick = () => invoke('reorder_instruction', { index: i, direction: -1 });

    const downBtn = document.createElement('button');
    downBtn.className = 'btn-icon';
    downBtn.appendChild(iconEl('chevron-down'));
    downBtn.title = 'Move down';
    downBtn.setAttribute('aria-label', 'Move down');
    downBtn.onclick = () => invoke('reorder_instruction', { index: i, direction: 1 });

    const removeBtn = document.createElement('button');
    removeBtn.className = 'btn-icon btn-danger';
    removeBtn.appendChild(iconEl('x'));
    removeBtn.title = 'Remove instruction';
    removeBtn.setAttribute('aria-label', 'Remove instruction');
    removeBtn.onclick = () => invoke('remove_instruction', { index: i });

    const insertAfterDd = dropdown(
        Object.keys(INSTRUCTION_TYPE_LABELS).map(t => ({ value: t, label: INSTRUCTION_TYPE_LABELS[t] })),
        '',
        insType => addInstructionAt(i + 1, insType),
        {
            iconOnly: true,
            triggerIcon: 'corner-down-right',
            className: 'btn-icon',
            ariaLabel: 'Insert instruction after this one',
            title: 'Insert instruction after this one',
            resetAfterSelect: true,
        }
    );

    controls.appendChild(upBtn);
    controls.appendChild(downBtn);
    controls.appendChild(removeBtn);
    controls.appendChild(insertAfterDd);
    row.appendChild(controls);
    return row;
}

function buildInstructionContent(content, i, ins, keyCaptureIdx, invalidBuffers, prevInvalidKeys) {
    const label = document.createElement('span');
    label.className = 'instruction-label';

    switch (ins.type) {
        case 'Wait': {
            label.textContent = 'Wait (ms):';
            const durBuf = getInvalidText(invalidBuffers, prevInvalidKeys, i, 'WaitDuration');
            const randBuf = getInvalidText(invalidBuffers, prevInvalidKeys, i, 'WaitRandomness');
            const durInput = numInput(durBuf?.text ?? String(ins.duration), durBuf?.invalid, durBuf?.isNew, v =>
                invoke('edit_instruction_field', { index: i, fieldId: 'WaitDuration', text: v }), i, 'WaitDuration');
            const randInput = numInput(randBuf?.text ?? String(ins.randomness), randBuf?.invalid, randBuf?.isNew, v =>
                invoke('edit_instruction_field', { index: i, fieldId: 'WaitRandomness', text: v }), i, 'WaitRandomness');
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
                invoke('edit_instruction', { index: i, instruction: { type: 'Text', text: v } }), i, 'Text');
            content.appendChild(label);
            content.appendChild(inp);
            break;
        }
        case 'Key': {
            label.textContent = 'Key:';
            const isCapturing = keyCaptureIdx === i;
            const captureBtn = document.createElement('button');
            captureBtn.className = 'btn-chip key-capture-btn' + (isCapturing ? ' capturing' : '');
            captureBtn.textContent = isCapturing ? 'Press any key…' : ins.key;
            captureBtn.onclick = () => invoke('start_key_capture', { index: i });

            const dirSel = directionSelect(ins.direction, dir =>
                invoke('edit_instruction', { index: i, instruction: { type: 'Key', key: ins.key, direction: dir } }));
            content.appendChild(label);
            content.appendChild(captureBtn);
            content.appendChild(dirSel);
            break;
        }
        case 'Button': {
            label.textContent = 'Mouse:';
            const buttons = ['Left', 'Right', 'Middle', 'Side', 'Extra'];
            const btnSel = enumSelect(buttons, ins.button, v =>
                invoke('edit_instruction', { index: i, instruction: { type: 'Button', button: v, direction: ins.direction } }));
            const dirSel = directionSelect(ins.direction, dir =>
                invoke('edit_instruction', { index: i, instruction: { type: 'Button', button: ins.button, direction: dir } }));
            content.appendChild(label);
            content.appendChild(btnSel);
            content.appendChild(dirSel);
            break;
        }
        case 'MoveMouse': {
            label.textContent = 'Move mouse:';
            const xBuf = getInvalidText(invalidBuffers, prevInvalidKeys, i, 'MoveMouseX');
            const yBuf = getInvalidText(invalidBuffers, prevInvalidKeys, i, 'MoveMouseY');
            const xInput = numInput(xBuf?.text ?? String(ins.x), xBuf?.invalid, xBuf?.isNew, v =>
                invoke('edit_instruction_field', { index: i, fieldId: 'MoveMouseX', text: v }), i, 'MoveMouseX');
            const yInput = numInput(yBuf?.text ?? String(ins.y), yBuf?.invalid, yBuf?.isNew, v =>
                invoke('edit_instruction_field', { index: i, fieldId: 'MoveMouseY', text: v }), i, 'MoveMouseY');
            xInput.placeholder = 'X';
            yInput.placeholder = 'Y';
            const coordSel = enumSelect(['Absolute', 'Relative'], ins.coordinate, v =>
                invoke('edit_instruction', { index: i, instruction: { type: 'MoveMouse', x: ins.x, y: ins.y, coordinate: v } }));
            content.appendChild(label);
            content.appendChild(xInput);
            content.appendChild(yInput);
            content.appendChild(coordSel);
            break;
        }
        case 'Scroll': {
            label.textContent = 'Scroll:';
            const amtBuf = getInvalidText(invalidBuffers, prevInvalidKeys, i, 'ScrollAmount');
            const amtInput = numInput(amtBuf?.text ?? String(ins.amount), amtBuf?.invalid, amtBuf?.isNew, v =>
                invoke('edit_instruction_field', { index: i, fieldId: 'ScrollAmount', text: v }), i, 'ScrollAmount');
            const axisSel = enumSelect(['Vertical', 'Horizontal'], ins.axis, v =>
                invoke('edit_instruction', { index: i, instruction: { type: 'Scroll', amount: ins.amount, axis: v } }));
            content.appendChild(label);
            content.appendChild(amtInput);
            content.appendChild(axisSel);
            break;
        }
        case 'Command': {
            label.textContent = 'Command:';
            const inp = textInput(ins.command, v =>
                invoke('edit_instruction', { index: i, instruction: { type: 'Command', command: v } }), i, 'Command');
            inp.placeholder = 'bash -c …';
            content.appendChild(label);
            content.appendChild(inp);
            break;
        }
        case 'Comment': {
            label.textContent = '//';
            const inp = textInput(ins.comment, v =>
                invoke('edit_instruction', { index: i, instruction: { type: 'Comment', comment: v } }), i, 'Comment');
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

function textInput(value, onChange, ix, field) {
    const inp = document.createElement('input');
    inp.type = 'text';
    inp.value = value;
    inp.style.flex = '1';
    if (ix != null && field != null) { inp.dataset.ix = String(ix); inp.dataset.field = field; }
    inp.addEventListener('input', () => onChange(inp.value));
    return inp;
}

function numInput(value, invalid, isNew, onChange, ix, field) {
    const inp = document.createElement('input');
    inp.type = 'text';
    inp.value = value;
    inp.style.width = '72px';
    if (invalid) {
        inp.classList.add('invalid');
        if (isNew) inp.classList.add('shake-once');
    }
    if (ix != null && field != null) { inp.dataset.ix = String(ix); inp.dataset.field = field; }
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
        case 'Key':       return { type: 'Key', key: 'KeyA', direction: 'Click' };
        case 'Button':    return { type: 'Button', button: 'Left', direction: 'Click' };
        case 'MoveMouse': return { type: 'MoveMouse', x: 0, y: 0, coordinate: 'Relative' };
        case 'Scroll':    return { type: 'Scroll', amount: 4, axis: 'Vertical' };
        case 'Command':   return { type: 'Command', command: '' };
        case 'Comment':   return { type: 'Comment', comment: '' };
        default:          return { type: 'Comment', comment: '' };
    }
}

async function addInstructionAt(index, type) {
    const ins = defaultInstruction(type);
    await invoke('add_instruction', { index, instruction: ins });
}

// Split button: left segment adds `addInstructionType` immediately, the
// chevron opens a picker that both changes the type and adds one right away.
function buildAddInstructionRow() {
    const row = document.getElementById('add-instruction-row');
    row.replaceChildren();

    const group = document.createElement('div');
    group.className = 'dd-split-group';

    addInstructionMainBtn = document.createElement('button');
    addInstructionMainBtn.className = 'btn-primary dd-split-main';
    addInstructionMainBtn.title = 'Add instruction at end';
    updateAddInstructionMainBtn();
    addInstructionMainBtn.onclick = () => {
        const len = state.current_macro?.instructions?.length ?? 0;
        addInstructionAt(len, addInstructionType);
    };

    const typeDropdown = dropdown(
        Object.keys(INSTRUCTION_TYPE_LABELS).map(t => ({ value: t, label: INSTRUCTION_TYPE_LABELS[t] })),
        addInstructionType,
        val => {
            addInstructionType = val;
            updateAddInstructionMainBtn();
            const len = state.current_macro?.instructions?.length ?? 0;
            addInstructionAt(len, val);
        },
        { iconOnly: true, triggerIcon: 'chevron-down', className: 'dd-split-chevron btn-primary', ariaLabel: 'Choose instruction type to add' }
    );

    group.appendChild(addInstructionMainBtn);
    group.appendChild(typeDropdown);
    row.appendChild(group);
}

function updateAddInstructionMainBtn() {
    setBtnContent(addInstructionMainBtn, {
        icon: INSTRUCTION_TYPE_ICONS[addInstructionType],
        text: `Add ${INSTRUCTION_TYPE_LABELS[addInstructionType]}`,
    });
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
    // Scroll listener for virtual list
    attachScrollListener();

    // Macro selector
    macroDropdown = dropdown([], '', val => {
        if (val === '') return;
        invoke('select_macro', { index: parseInt(val) });
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

    // Add instruction at end (split button: click adds the current type, chevron picks a new type)
    buildAddInstructionRow();

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
