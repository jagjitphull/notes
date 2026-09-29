<script setup lang="ts">
import { computed } from "vue";
import { icons, type IconName } from "./icons";

const props = defineProps<{ name: IconName }>();

// v-html is safe here: `markup` only ever resolves to one of this module's
// own hand-authored constants in icons.ts, never anything derived from
// user input or external data.
const markup = computed(() => icons[props.name]);
</script>

<template>
  <svg
    viewBox="0 0 20 20"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    v-html="markup"
  />
</template>
