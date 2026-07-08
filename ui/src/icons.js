// Lucide icons (https://lucide.dev), MIT/ISC licensed — imported as raw SVG
// source via lucide-static so markup stays byte-identical to upstream.

import chevronUp from 'lucide-static/icons/chevron-up.svg?raw';
import chevronDown from 'lucide-static/icons/chevron-down.svg?raw';
import trash from 'lucide-static/icons/trash-2.svg?raw';
import plus from 'lucide-static/icons/plus.svg?raw';
import x from 'lucide-static/icons/x.svg?raw';
import settings from 'lucide-static/icons/settings.svg?raw';
import play from 'lucide-static/icons/play.svg?raw';
import repeat from 'lucide-static/icons/repeat.svg?raw';
import circle from 'lucide-static/icons/circle.svg?raw';
import square from 'lucide-static/icons/square.svg?raw';
import pause from 'lucide-static/icons/pause.svg?raw';
import cornerUpLeft from 'lucide-static/icons/corner-up-left.svg?raw';
import cornerUpRight from 'lucide-static/icons/corner-up-right.svg?raw';
import save from 'lucide-static/icons/save.svg?raw';
import arrowLeft from 'lucide-static/icons/arrow-left.svg?raw';
import key from 'lucide-static/icons/key.svg?raw';
import alertTriangle from 'lucide-static/icons/triangle-alert.svg?raw';
import sun from 'lucide-static/icons/sun.svg?raw';
import moon from 'lucide-static/icons/moon.svg?raw';
import refreshCw from 'lucide-static/icons/refresh-cw.svg?raw';
import server from 'lucide-static/icons/server.svg?raw';
import inbox from 'lucide-static/icons/inbox.svg?raw';

export const ICONS = {
    'chevron-up': chevronUp,
    'chevron-down': chevronDown,
    'trash': trash,
    'plus': plus,
    'x': x,
    'settings': settings,
    'play': play,
    'repeat': repeat,
    'circle': circle,
    'square': square,
    'pause': pause,
    'corner-up-left': cornerUpLeft,
    'corner-up-right': cornerUpRight,
    'save': save,
    'arrow-left': arrowLeft,
    'key': key,
    'alert-triangle': alertTriangle,
    'sun': sun,
    'moon': moon,
    'refresh-cw': refreshCw,
    'server': server,
    'inbox': inbox,
};

/**
 * Builds a DOM node containing the named icon's SVG markup.
 * ICONS values come from lucide-static's bundled SVG files at build time
 * (never user input), so assigning via innerHTML here is safe.
 */
export function iconEl(name) {
    const span = document.createElement('span');
    span.className = 'btn-icon-glyph';
    span.innerHTML = ICONS[name] ?? '';
    return span;
}

/**
 * Sets a button's content to an icon + optional text label, clearing whatever
 * was there before (used in render functions that rebuild button content on
 * every state push).
 */
export function setBtnContent(btn, { icon, text } = {}) {
    btn.replaceChildren();
    if (icon) btn.appendChild(iconEl(icon));
    if (text) {
        const label = document.createElement('span');
        label.textContent = text;
        btn.appendChild(label);
    }
}
