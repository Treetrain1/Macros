<script setup lang="ts">
import { Inbox } from 'lucide-vue-next';
</script>

<template>
  <div class="empty-state">
    <Inbox />
    <p>No macro selected</p>
    <span>Choose a macro above, or create a new one to get started.</span>
  </div>
</template>
