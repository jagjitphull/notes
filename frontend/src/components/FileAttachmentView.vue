<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { NodeViewWrapper, nodeViewProps } from "@tiptap/vue-3";
import Icon from "./icons/Icon.vue";
import { getAttachmentSize, openAttachment } from "../api";

const props = defineProps(nodeViewProps);

const path = computed(() => (props.node.attrs.path as string) ?? "");
const filename = computed(() => path.value.split("/").pop() || path.value);

// Always re-stat on mount rather than trusting a size baked into the
// node/markdown at insert time - the file on disk is the single source of
// truth, and this way a reopened note shows the same thing a freshly
// inserted one does, with no separate "did this go stale" question.
const size = ref<number | null>(null);
const broken = ref(false);

onMounted(async () => {
  try {
    size.value = await getAttachmentSize(path.value);
  } catch {
    broken.value = true;
  }
});

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function onClick() {
  if (broken.value) return;
  openAttachment(path.value);
}
</script>

<template>
  <NodeViewWrapper
    as="span"
    class="file-attachment"
    :class="{ 'file-attachment--broken': broken }"
    :title="filename"
    @click="onClick"
  >
    <Icon name="attachment" class="file-attachment-icon" />
    <span class="file-attachment-name">{{ filename }}</span>
    <span v-if="size !== null" class="file-attachment-size">{{ formatSize(size) }}</span>
  </NodeViewWrapper>
</template>

<style scoped>
.file-attachment {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border-radius: 6px;
  padding: 2px 8px 2px 6px;
  color: var(--text-primary);
  background: var(--bg-selected);
  border: 1px solid var(--border);
  cursor: pointer;
  font-size: 0.92em;
  vertical-align: middle;
}

.file-attachment:hover {
  background: var(--bg-hover);
}

.file-attachment-icon {
  width: 13px;
  height: 13px;
  flex: 0 0 auto;
  color: var(--text-secondary);
}

.file-attachment-name {
  max-width: 240px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-attachment-size {
  color: var(--text-tertiary);
  font-size: 0.85em;
}

.file-attachment--broken {
  color: var(--text-tertiary);
  background: transparent;
  border-style: dashed;
  cursor: default;
}
</style>
