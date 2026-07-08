// Custom listbox/dropdown replacing native <select>. WebKitGTK (the Linux
// Tauri webview) closes native <select> popups on the same mousedown that
// opens them, making their contents unreadable — there is no fix for that
// from outside the browser engine, so every dropdown in the app is built
// from real DOM instead.
import { iconEl } from './icons.js';

let portal = null;
function getPortal() {
    if (!portal) portal = document.getElementById('dd-portal');
    return portal;
}

// Only one dropdown panel is ever open at a time.
let active = null; // { root, panel, close }

export function closeAllDropdowns() {
    if (active) active.close();
}

function normalizeOptions(options) {
    return options.map(o => (typeof o === 'string' ? { value: o, label: o } : o));
}

function positionPanel(panel, trigger) {
    const rect = trigger.getBoundingClientRect();
    const panelH = panel.offsetHeight;
    const spaceBelow = window.innerHeight - rect.bottom;
    const openUp = spaceBelow < panelH + 8 && rect.top > spaceBelow;

    panel.style.left = Math.max(4, Math.min(rect.left, window.innerWidth - panel.offsetWidth - 4)) + 'px';
    if (openUp) {
        panel.style.top = '';
        panel.style.bottom = (window.innerHeight - rect.top + 4) + 'px';
    } else {
        panel.style.bottom = '';
        panel.style.top = (rect.bottom + 4) + 'px';
    }
    panel.style.minWidth = rect.width + 'px';
}

/**
 * Builds a custom dropdown control.
 * @param {Array<string|{value,label}>} options
 * @param {string} current - initial selected value
 * @param {(value: string) => void} onChange
 * @param {object} opts
 * @param {string} [opts.placeholder] - shown when current is '' / not found
 * @param {string} [opts.className] - extra class(es) on the trigger button
 * @param {boolean} [opts.iconOnly] - trigger shows only an icon, never a label
 * @param {string} [opts.triggerIcon] - icon name for the trigger (defaults to chevron-down for normal mode)
 * @param {string} [opts.ariaLabel]
 * @param {boolean} [opts.resetAfterSelect] - after commit, revert to placeholder instead of showing the picked value (used by "Insert after")
 */
export function dropdown(options, current, onChange, opts = {}) {
    let optList = normalizeOptions(options);
    let value = current ?? '';

    const root = document.createElement('div');
    root.className = 'dd';

    const trigger = document.createElement('button');
    trigger.type = 'button';
    trigger.className = 'dd-trigger' + (opts.iconOnly ? ' dd-trigger-icon' : '') + (opts.className ? ' ' + opts.className : '');
    trigger.setAttribute('aria-haspopup', 'listbox');
    trigger.setAttribute('aria-expanded', 'false');
    if (opts.ariaLabel) trigger.setAttribute('aria-label', opts.ariaLabel);
    if (opts.title) trigger.title = opts.title;
    root.appendChild(trigger);

    let panel = null;
    let activeIndex = -1;

    function labelFor(val) {
        const found = optList.find(o => String(o.value) === String(val));
        return found ? found.label : (opts.placeholder ?? '');
    }

    function renderTrigger() {
        trigger.replaceChildren();
        if (opts.iconOnly) {
            trigger.appendChild(iconEl(opts.triggerIcon ?? 'chevron-down'));
        } else {
            const label = document.createElement('span');
            label.className = 'dd-trigger-label';
            const text = labelFor(value);
            label.textContent = text || (opts.placeholder ?? '');
            if (!text && opts.placeholder) label.classList.add('dd-trigger-placeholder');
            trigger.appendChild(label);
            trigger.appendChild(iconEl('chevron-down'));
        }
    }
    renderTrigger();

    function isOpen() { return panel != null; }

    function close() {
        if (!panel) return;
        panel.remove();
        panel = null;
        activeIndex = -1;
        trigger.setAttribute('aria-expanded', 'false');
        trigger.removeAttribute('aria-activedescendant');
        document.removeEventListener('mousedown', onOutsideMouseDown, true);
        window.removeEventListener('scroll', onScrollOrResize, true);
        window.removeEventListener('resize', onScrollOrResize);
        if (active && active.root === root) active = null;
    }

    function onOutsideMouseDown(e) {
        if (root.contains(e.target) || (panel && panel.contains(e.target))) return;
        if (panel) {
            const r = panel.getBoundingClientRect();
            if (e.clientX >= r.left && e.clientX <= r.right &&
                e.clientY >= r.top && e.clientY <= r.bottom) return;
        }
        close();
    }
    function onScrollOrResize() {
        if (!panel) return;
        const tr = trigger.getBoundingClientRect();
        const inView = tr.bottom >= 0 && tr.top <= window.innerHeight;
        if (!inView) close();
    }

    function commit(val) {
        value = val;
        renderTrigger();
        close();
        trigger.focus();
        if (opts.resetAfterSelect) {
            value = '';
            renderTrigger();
        }
        onChange(val);
    }

    function highlight(idx) {
        if (!panel) return;
        const rows = panel.querySelectorAll('.dd-option');
        rows.forEach(r => r.setAttribute('aria-selected', 'false'));
        activeIndex = Math.max(0, Math.min(idx, rows.length - 1));
        const row = rows[activeIndex];
        if (row) {
            row.setAttribute('aria-selected', 'true');
            row.id = row.id || `dd-opt-${Math.random().toString(36).slice(2)}`;
            trigger.setAttribute('aria-activedescendant', row.id);
            row.scrollIntoView({ block: 'nearest' });
        }
    }

    function buildPanelRows() {
        panel.replaceChildren();
        optList.forEach((o, idx) => {
            const row = document.createElement('div');
            row.className = 'dd-option';
            row.setAttribute('role', 'option');
            row.dataset.value = o.value;
            row.textContent = o.label;
            if (String(o.value) === String(value)) row.classList.add('dd-option-current');
            row.addEventListener('click', () => commit(o.value));
            row.addEventListener('mouseenter', () => highlight(idx));
            panel.appendChild(row);
        });
    }

    function open() {
        if (active) active.close();
        panel = document.createElement('div');
        panel.className = 'dd-panel';
        panel.setAttribute('role', 'listbox');
        buildPanelRows();
        getPortal().appendChild(panel);
        positionPanel(panel, trigger);
        trigger.setAttribute('aria-expanded', 'true');

        const currentIdx = optList.findIndex(o => String(o.value) === String(value));
        highlight(currentIdx >= 0 ? currentIdx : 0);

        // Registered inside the opening click handler so the mousedown that
        // opened the panel is never re-seen as an "outside" close.
        document.addEventListener('mousedown', onOutsideMouseDown, true);
        window.addEventListener('scroll', onScrollOrResize, true);
        window.addEventListener('resize', onScrollOrResize);
        active = { root, panel, close };
    }

    trigger.addEventListener('click', () => {
        if (isOpen()) close();
        else open();
    });

    trigger.addEventListener('keydown', e => {
        if (!isOpen()) {
            if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
                e.preventDefault();
                open();
            }
            return;
        }
        if (e.key === 'Escape') {
            e.preventDefault();
            close();
        } else if (e.key === 'ArrowDown') {
            e.preventDefault();
            highlight(activeIndex + 1);
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            highlight(activeIndex - 1);
        } else if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            if (activeIndex >= 0 && optList[activeIndex]) commit(optList[activeIndex].value);
        } else if (e.key === 'Tab') {
            close();
        }
    });

    Object.defineProperty(root, 'ddValue', {
        get: () => value,
        set: v => { value = v ?? ''; renderTrigger(); },
    });
    root.ddSetOptions = newOptions => {
        optList = normalizeOptions(newOptions);
        renderTrigger();
        if (isOpen()) buildPanelRows();
    };
    root.ddFocus = () => trigger.focus();

    return root;
}
