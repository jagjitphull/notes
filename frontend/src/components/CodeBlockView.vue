<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NodeViewWrapper, NodeViewContent, nodeViewProps } from "@tiptap/vue-3";
import katex from "katex";
import mermaid from "mermaid";
import { useTheme } from "../composables/useTheme";

const props = defineProps(nodeViewProps);

const language = computed(() => (props.node.attrs.language as string | null) ?? "");
const code = computed(() => props.node.textContent);

const { preference, systemPrefersDark } = useTheme();
const isDark = computed(
  () => preference.value === "dark" || (preference.value === "system" && systemPrefersDark.value),
);

const previewHtml = ref("");
const previewError = ref("");

let renderToken = 0;

// Re-renders the preview whenever the language, the block's own text, or
// the app's dark/light theme changes (typing inside it fires this via
// the node view's own update, not a parent watcher - see onUpdate
// below). mermaid.render is async, so a token guards against a slow
// render from an earlier keystroke clobbering a newer one that finished
// first.
async function renderPreview() {
  const myToken = ++renderToken;
  if (language.value === "mermaid") {
    try {
      mermaid.initialize({ startOnLoad: false, theme: isDark.value ? "dark" : "default" });
      const id = `mermaid-preview-${Math.random().toString(36).slice(2)}`;
      const { svg } = await mermaid.render(id, code.value.trim() || "graph TD\nA");
      if (myToken !== renderToken) return;
      previewHtml.value = svg;
      previewError.value = "";
    } catch (e) {
      if (myToken !== renderToken) return;
      previewHtml.value = "";
      previewError.value = e instanceof Error ? e.message : String(e);
    }
  } else if (language.value === "math") {
    try {
      previewHtml.value = katex.renderToString(code.value, { throwOnError: true, displayMode: true });
      previewError.value = "";
    } catch (e) {
      previewHtml.value = "";
      previewError.value = e instanceof Error ? e.message : String(e);
    }
  } else {
    previewHtml.value = "";
    previewError.value = "";
  }
}

watch([language, code, isDark], renderPreview, { immediate: true });
</script>

<template>
  <NodeViewWrapper class="code-block-view">
    <pre><NodeViewContent as="code" /></pre>
    <div v-if="previewHtml" class="code-block-preview" v-html="previewHtml" />
    <p v-else-if="previewError" class="code-block-preview-error">{{ previewError }}</p>
  </NodeViewWrapper>
</template>

<style scoped>
.code-block-preview {
  margin-top: 8px;
  padding: 10px 12px;
  border-radius: 6px;
  background: var(--bg-hover);
  overflow-x: auto;
  text-align: center;
}

.code-block-preview :deep(svg) {
  max-width: 100%;
}

.code-block-preview-error {
  margin: 8px 0 0;
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--bg-hover);
  color: var(--danger);
  font-family: var(--mono, ui-monospace, monospace);
  font-size: 12px;
  white-space: pre-wrap;
}
</style>
