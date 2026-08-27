<script setup lang="ts">
import { computed } from 'vue';
import { Clock } from 'lucide-vue-next';
import { AppDropdown } from 'blockstitch';
import { MONTH_OPTIONS, SCHEDULE_MODE_OPTIONS, WEEKDAY_OPTIONS, scheduleTimeValue, withScheduleMode, withTimeValue } from '../../../timeSchedule';
import type { InstructionDto, TimeScheduleDto, WeekdayDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'WhenTime' }> }>();

function onModeChange(v: string) {
  props.instruction.schedule = withScheduleMode(props.instruction.schedule, v as TimeScheduleDto['kind']);
}

function onWeekdayChange(v: string) {
  const s = props.instruction.schedule;
  if (s.kind !== 'Weekly') return;
  s.weekday = v as WeekdayDto;
}

function clampDay(raw: string): number {
  return Math.min(31, Math.max(1, Math.round(Number(raw)) || 1));
}

function onDayChange(e: Event) {
  const s = props.instruction.schedule;
  if (s.kind !== 'Monthly' && s.kind !== 'Yearly') return;
  s.day = clampDay((e.target as HTMLInputElement).value);
}

function onMonthChange(v: string) {
  const s = props.instruction.schedule;
  if (s.kind !== 'Yearly') return;
  s.month = Number(v);
}

function onTimeChange(e: Event) {
  props.instruction.schedule = withTimeValue(props.instruction.schedule, (e.target as HTMLInputElement).value);
}

const timeValue = computed(() => scheduleTimeValue(props.instruction.schedule));
</script>

<template>
  <Clock />
  <span class="instruction-label when-ran-label">When</span>
  <AppDropdown
    :options="SCHEDULE_MODE_OPTIONS"
    :model-value="instruction.schedule.kind"
    class-name="dd-compact"
    @update:model-value="onModeChange"
  />
  <template v-if="instruction.schedule.kind === 'Weekly'">
    <AppDropdown
      :options="WEEKDAY_OPTIONS"
      :model-value="instruction.schedule.weekday"
      class-name="dd-compact"
      @update:model-value="onWeekdayChange"
    />
  </template>
  <template v-else-if="instruction.schedule.kind === 'Monthly'">
    <span class="instruction-label">on day</span>
    <input type="number" min="1" max="31" class="when-time-day-input" :value="instruction.schedule.day" @change="onDayChange">
  </template>
  <template v-else-if="instruction.schedule.kind === 'Yearly'">
    <span class="instruction-label">on</span>
    <AppDropdown
      :options="MONTH_OPTIONS"
      :model-value="String(instruction.schedule.month)"
      class-name="dd-compact"
      @update:model-value="onMonthChange"
    />
    <input type="number" min="1" max="31" class="when-time-day-input" :value="instruction.schedule.day" @change="onDayChange">
  </template>
  <span class="instruction-label">at</span>
  <input type="time" class="when-time-time-input" :value="timeValue" @change="onTimeChange">
</template>
