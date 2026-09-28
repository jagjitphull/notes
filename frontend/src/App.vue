<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

interface DbHealth {
  folder_count: number;
  note_count: number;
  tag_count: number;
}

const health = ref<DbHealth | null>(null);
const error = ref<string | null>(null);

onMounted(async () => {
  try {
    health.value = await invoke<DbHealth>("db_health_check");
  } catch (e) {
    error.value = String(e);
  }
});
</script>

<template>
  <main class="scaffold-check">
    <h1>Notes</h1>
    <p v-if="health">
      Database connected — {{ health.folder_count }} folder(s),
      {{ health.note_count }} note(s), {{ health.tag_count }} tag(s).
    </p>
    <p v-else-if="error" class="error">Database error: {{ error }}</p>
    <p v-else>Connecting to database…</p>
  </main>
</template>

<style scoped>
.scaffold-check {
  font-family: system-ui, sans-serif;
  padding: 2rem;
}

.error {
  color: #d33;
}
</style>
