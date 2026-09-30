<script setup lang="ts">
import { computed } from "vue";
import { NodeViewWrapper, nodeViewProps } from "@tiptap/vue-3";
import type { NoteLinkOptions } from "../tiptap/noteLink";

const props = defineProps(nodeViewProps);

const title = computed(() => (props.node.attrs.title as string) ?? "");

const options = computed(() => props.extension.options as NoteLinkOptions);
const resolved = computed(() => options.value.isResolved(title.value));

function onClick() {
  if (resolved.value) options.value.onNavigate(title.value);
}
</script>

<template>
  <NodeViewWrapper
    as="span"
    class="note-link"
    :class="{ 'note-link--broken': !resolved }"
    :title="title"
    @click="onClick"
    >{{ title }}</NodeViewWrapper
  >
</template>

<style scoped>
.note-link {
  border-radius: 3px;
  padding: 0 3px;
  color: var(--accent-blue-text);
  background: var(--bg-selected);
  cursor: pointer;
}

.note-link:hover {
  text-decoration: underline;
}

.note-link--broken {
  color: var(--text-tertiary);
  background: transparent;
  border: 1px dashed var(--border);
  cursor: default;
}

.note-link--broken:hover {
  text-decoration: none;
}
</style>
