import { ref } from "vue";

// A drawing's re-edit flow (FileAttachmentView.vue, inside a Tiptap node
// view) needs to know which note it's saving the re-drawn PNG/JSON
// attachment under. Editor.vue's own `props.note` isn't reachable from
// there: Tiptap's extensions - and the node views they render - are set
// up once and reused as the editor switches between notes, not
// recreated per note, so it can't be threaded through as a static
// extension option. A small shared ref, kept in sync by Editor.vue
// (the one place that always knows the current note), is simpler and
// more robust here than relying on provide/inject reaching into a node
// view's own render tree.
export const currentNoteId = ref<string | null>(null);
