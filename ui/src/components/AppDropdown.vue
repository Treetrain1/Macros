<script setup lang="ts">
// Custom listbox/dropdown replacing native <select>. WebKitGTK (the Linux
// Tauri webview) closes native <select> popups on the same mousedown that
// opens them, making their contents unreadable — there is no fix for that
// from outside the browser engine, so every dropdown in the app is built
// from real DOM instead. This is a webview bug, unrelated to the JS
// framework, so it persists verbatim in the Vue port.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { ChevronDown } from 'lucide-vue-next';
import { ICONS, type IconName } from '../icons';
import { registerOpen, unregisterOpen } from '../dropdownRegistry';

type Option = string | { value: string; label: string };

const props = defineProps<{
  options: Option[];
  modelValue: string;
  placeholder?: string;
  className?: string;
  iconOnly?: boolean;
  triggerIcon?: IconName;
  ariaLabel?: string;
  ariaLabelledby?: string;
  resetAfterSelect?: boolean;
  title?: string;
}>();

const emit = defineEmits<{ 'update:modelValue': [value: string] }>();

const normalized = computed(() =>
  props.options.map(o => (typeof o === 'string' ? { value: o, label: o } : o)),
);

// Local display value: normally mirrors modelValue, except resetAfterSelect
// mode reverts the trigger's label back to the placeholder right after a
// commit while the emitted modelValue itself still carries the picked value.
const displayValue = ref(props.modelValue);
watch(() => props.modelValue, v => { displayValue.value = v; });

const currentLabel = computed(() => {
  const found = normalized.value.find(o => String(o.value) === String(displayValue.value));
  return found ? found.label : '';
});

const triggerIconComponent = computed(() => ICONS[props.triggerIcon ?? 'chevron-down']);

const isOpen = ref(false);
const activeIndex = ref(-1);
const triggerRef = ref<HTMLButtonElement | null>(null);
const panelRef = ref<HTMLDivElement | null>(null);
const panelStyle = ref<{ left?: string; top?: string; bottom?: string; minWidth?: string }>({});

function positionPanel() {
  const trigger = triggerRef.value;
  const panel = panelRef.value;
  if (!trigger || !panel) return;
  const rect = trigger.getBoundingClientRect();
  const panelH = panel.offsetHeight;
  const spaceBelow = window.innerHeight - rect.bottom;
  const openUp = spaceBelow < panelH + 8 && rect.top > spaceBelow;

  const left = Math.max(4, Math.min(rect.left, window.innerWidth - panel.offsetWidth - 4));
  panelStyle.value = openUp
    ? { left: `${left}px`, bottom: `${window.innerHeight - rect.top + 4}px`, minWidth: `${rect.width}px` }
    : { left: `${left}px`, top: `${rect.bottom + 4}px`, minWidth: `${rect.width}px` };
}

function onOutsideMouseDown(e: MouseEvent) {
  const target = e.target as Node;
  if (triggerRef.value?.contains(target) || panelRef.value?.contains(target)) return;
  close();
}

function onScrollOrResize() {
  if (!isOpen.value || !triggerRef.value) return;
  const r = triggerRef.value.getBoundingClientRect();
  const inView = r.bottom >= 0 && r.top <= window.innerHeight;
  if (!inView) close();
}

async function open() {
  registerOpen(close);
  isOpen.value = true;
  const currentIdx = normalized.value.findIndex(o => String(o.value) === String(displayValue.value));
  activeIndex.value = currentIdx >= 0 ? currentIdx : 0;
  await nextTick();
  positionPanel();
  // Registered after the opening click has already been dispatched, so the
  // mousedown that opened the panel is never re-seen as an "outside" close.
  document.addEventListener('mousedown', onOutsideMouseDown, true);
  window.addEventListener('scroll', onScrollOrResize, true);
  window.addEventListener('resize', onScrollOrResize);
}

function close() {
  if (!isOpen.value) return;
  isOpen.value = false;
  activeIndex.value = -1;
  document.removeEventListener('mousedown', onOutsideMouseDown, true);
  window.removeEventListener('scroll', onScrollOrResize, true);
  window.removeEventListener('resize', onScrollOrResize);
  unregisterOpen(close);
}

function commit(value: string) {
  displayValue.value = value;
  close();
  triggerRef.value?.focus();
  if (props.resetAfterSelect) displayValue.value = '';
  emit('update:modelValue', value);
}

function highlight(idx: number) {
  activeIndex.value = Math.max(0, Math.min(idx, normalized.value.length - 1));
}

function onTriggerClick() {
  if (isOpen.value) close();
  else open();
}

function onTriggerKeydown(e: KeyboardEvent) {
  if (!isOpen.value) {
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
    highlight(activeIndex.value + 1);
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    highlight(activeIndex.value - 1);
  } else if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault();
    const opt = normalized.value[activeIndex.value];
    if (opt) commit(opt.value);
  } else if (e.key === 'Tab') {
    close();
  }
}

onBeforeUnmount(() => close());

defineExpose({ focus: () => triggerRef.value?.focus() });
</script>

<template>
  <div class="dd">
    <button
      ref="triggerRef"
      type="button"
      class="dd-trigger"
      :class="[{ 'dd-trigger-icon': iconOnly }, className]"
      aria-haspopup="listbox"
      :aria-expanded="isOpen"
      :aria-label="ariaLabel"
      :aria-labelledby="ariaLabelledby"
      :title="title"
      @click="onTriggerClick"
      @keydown="onTriggerKeydown"
    >
      <template v-if="iconOnly">
        <component :is="triggerIconComponent" />
      </template>
      <template v-else>
        <span class="dd-trigger-label" :class="{ 'dd-trigger-placeholder': !currentLabel }">
          {{ currentLabel || placeholder || '' }}
        </span>
        <ChevronDown />
      </template>
    </button>

    <Teleport to="#dd-portal">
      <div v-if="isOpen" ref="panelRef" class="dd-panel" role="listbox" :style="panelStyle">
        <div
          v-for="(o, idx) in normalized"
          :key="o.value"
          class="dd-option"
          role="option"
          :class="{ 'dd-option-current': String(o.value) === String(displayValue) }"
          :aria-selected="idx === activeIndex"
          @click="commit(o.value)"
          @mouseenter="highlight(idx)"
        >
          {{ o.label }}
        </div>
      </div>
    </Teleport>
  </div>
</template>
