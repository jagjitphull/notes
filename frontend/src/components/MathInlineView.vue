<script setup lang="ts">
import { computed } from "vue";
import { NodeViewWrapper, nodeViewProps } from "@tiptap/vue-3";
import katex from "katex";

const props = defineProps(nodeViewProps);

const latex = computed(() => (props.node.attrs.latex as string) ?? "");

const rendered = computed(() => {
  try {
    return { html: katex.renderToString(latex.value, { throwOnError: true, displayMode: false }), error: "" };
  } catch (e) {
    return { html: "", error: e instanceof Error ? e.message : String(e) };
  }
});
</script>

<template>
  <NodeViewWrapper
    as="span"
    class="math-inline"
    :class="{ 'math-inline--error': rendered.error }"
    :title="rendered.error || latex"
  >
    <span v-if="rendered.html" v-html="rendered.html" />
    <span v-else>${{ latex }}$</span>
  </NodeViewWrapper>
</template>

<style scoped>
.math-inline {
  display: inline-block;
}

.math-inline--error {
  color: var(--danger);
  font-family: var(--mono, ui-monospace, monospace);
  font-size: 0.9em;
  border-bottom: 1px dashed var(--danger);
}
</style>
