<script setup lang="ts">
import { state } from '../store';
import MacroSelector from './MacroSelector.vue';
import RunControls from './RunControls.vue';
import EmptyState from './EmptyState.vue';
import MacroEditor from './MacroEditor.vue';
</script>

<template>
  <div id="main-page">
    <MacroSelector />
    <RunControls />
    <EmptyState v-if="!state.current_macro" />
    <MacroEditor v-else />
  </div>
</template>
