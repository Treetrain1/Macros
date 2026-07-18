<script setup lang="ts">
import SettingsHeader from './SettingsHeader.vue';
import WarningBanners from './WarningBanners.vue';
import GlobalHotkeysSection from './GlobalHotkeysSection.vue';
import PerMacroHotkeysSection from './PerMacroHotkeysSection.vue';
import TcpServerSection from './TcpServerSection.vue';
import ImportExportSection from './ImportExportSection.vue';
import UpdatesSection from './UpdatesSection.vue';
</script>

<template>
  <div id="settings-page">
    <SettingsHeader />
    <div class="settings-scroll">
      <WarningBanners />
      <GlobalHotkeysSection />
      <PerMacroHotkeysSection />
      <TcpServerSection />
      <ImportExportSection />
      <UpdatesSection />
    </div>
  </div>
</template>
