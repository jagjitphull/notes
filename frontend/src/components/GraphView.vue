<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { extractLinkedTitles } from "../tiptap/noteLink";
import type { Note } from "../types";

const props = defineProps<{
  notes: Note[];
}>();

const emit = defineEmits<{
  selectNote: [id: string];
}>();

const { t } = useI18n();

const WIDTH = 900;
const HEIGHT = 640;
const PADDING = 50;
const ITERATIONS = 250;

const graphNotes = computed(() => props.notes.filter((n) => !n.deletedAt && !n.isTemplate));

const titleIndex = computed(() => {
  const map = new Map<string, string>();
  for (const n of graphNotes.value) {
    const key = n.title.trim().toLowerCase();
    if (key && !map.has(key)) map.set(key, n.id);
  }
  return map;
});

interface GraphEdge {
  source: string;
  target: string;
}

const edges = computed<GraphEdge[]>(() => {
  const result: GraphEdge[] = [];
  const seen = new Set<string>();
  for (const n of graphNotes.value) {
    for (const title of extractLinkedTitles(n.plaintextContent)) {
      const targetId = titleIndex.value.get(title.trim().toLowerCase());
      if (!targetId || targetId === n.id) continue;
      const key = [n.id, targetId].sort().join("|");
      if (seen.has(key)) continue;
      seen.add(key);
      result.push({ source: n.id, target: targetId });
    }
  }
  return result;
});

// How many other notes each note links to or is linked from - nodes with
// more connections render as slightly larger circles, a simple visual cue
// for "hub" notes without needing a separate legend.
const degree = computed(() => {
  const counts = new Map<string, number>();
  for (const e of edges.value) {
    counts.set(e.source, (counts.get(e.source) ?? 0) + 1);
    counts.set(e.target, (counts.get(e.target) ?? 0) + 1);
  }
  return counts;
});

interface LayoutNode {
  id: string;
  title: string;
  x: number;
  y: number;
}

interface SimNode {
  id: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
}

// A Fruchterman-Reingold-style force layout - not a live, draggable
// simulation, but run incrementally (a handful of iterations per animation
// frame, below) rather than all at once, so it never blocks the main
// thread and the result settles into place visibly instead of teleporting.
function stepOnce(sim: SimNode[], byId: Map<string, SimNode>, edgeList: GraphEdge[]) {
  for (let i = 0; i < sim.length; i++) {
    for (let j = i + 1; j < sim.length; j++) {
      const a = sim[i];
      const b = sim[j];
      const dx = a.x - b.x;
      const dy = a.y - b.y;
      const distSq = Math.max(dx * dx + dy * dy, 1);
      const dist = Math.sqrt(distSq);
      const force = 6000 / distSq;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;
      a.vx += fx;
      a.vy += fy;
      b.vx -= fx;
      b.vy -= fy;
    }
  }

  for (const e of edgeList) {
    const a = byId.get(e.source);
    const b = byId.get(e.target);
    if (!a || !b) continue;
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const dist = Math.sqrt(dx * dx + dy * dy) || 1;
    const idealLength = 130;
    const force = (dist - idealLength) * 0.02;
    const fx = (dx / dist) * force;
    const fy = (dy / dist) * force;
    a.vx += fx;
    a.vy += fy;
    b.vx -= fx;
    b.vy -= fy;
  }

  for (const n of sim) {
    n.vx += (WIDTH / 2 - n.x) * 0.002;
    n.vy += (HEIGHT / 2 - n.y) * 0.002;
    n.vx *= 0.82;
    n.vy *= 0.82;
    n.x += n.vx;
    n.y += n.vy;
  }
}

const nodes = ref<LayoutNode[]>([]);
let animationFrame: number | null = null;

function cancelLayout() {
  if (animationFrame !== null) {
    cancelAnimationFrame(animationFrame);
    animationFrame = null;
  }
}

function startLayout() {
  cancelLayout();
  const list = graphNotes.value;
  if (list.length === 0) {
    nodes.value = [];
    return;
  }

  const sim: SimNode[] = list.map((n, i) => {
    const angle = (i / list.length) * Math.PI * 2;
    const r = Math.min(WIDTH, HEIGHT) / 3;
    return { id: n.id, x: WIDTH / 2 + Math.cos(angle) * r, y: HEIGHT / 2 + Math.sin(angle) * r, vx: 0, vy: 0 };
  });
  const byId = new Map(sim.map((n) => [n.id, n]));
  const edgeList = edges.value;
  const titleById = new Map(list.map((n) => [n.id, n.title]));

  function publish() {
    nodes.value = sim.map((n) => ({
      id: n.id,
      title: titleById.get(n.id) || t("common.newNote"),
      x: Math.min(Math.max(n.x, PADDING), WIDTH - PADDING),
      y: Math.min(Math.max(n.y, PADDING), HEIGHT - PADDING),
    }));
  }

  // A few physics steps per frame keeps each frame well under 16ms (the
  // graph pane and the rest of the UI both stay responsive while it
  // settles), even for vault sizes well beyond what this app expects.
  const STEPS_PER_FRAME = 4;
  let iter = 0;

  function tick() {
    for (let s = 0; s < STEPS_PER_FRAME && iter < ITERATIONS; s++, iter++) {
      stepOnce(sim, byId, edgeList);
    }
    publish();
    animationFrame = iter < ITERATIONS ? requestAnimationFrame(tick) : null;
  }

  tick();
}

// Re-lays-out from scratch only when the actual node/edge set changes -
// not on every unrelated note edit (pin, tags, word count, ...), which
// would otherwise re-trigger the full physics simulation on every autosave
// while the graph happens to be open.
const topologyKey = computed(() => {
  const ids = graphNotes.value
    .map((n) => n.id)
    .sort()
    .join(",");
  const edgeKey = edges.value
    .map((e) => [e.source, e.target].sort().join("-"))
    .sort()
    .join(",");
  return `${ids}::${edgeKey}`;
});
watch(topologyKey, startLayout, { immediate: true });

// A rename that doesn't change the link graph (most renames) just needs
// its label patched in place, not a full re-layout.
watch(
  () => graphNotes.value.map((n) => `${n.id}:${n.title}`).join(","),
  () => {
    if (animationFrame !== null) return; // a fresh layout is already in flight and will use current titles
    const titleById = new Map(graphNotes.value.map((n) => [n.id, n.title]));
    nodes.value = nodes.value.map((n) => ({ ...n, title: titleById.get(n.id) || t("common.newNote") }));
  },
);

onBeforeUnmount(cancelLayout);

const positionById = computed(() => new Map(nodes.value.map((n) => [n.id, n])));

function radiusFor(id: string): number {
  const d = degree.value.get(id) ?? 0;
  return Math.min(5 + d * 1.5, 14);
}
</script>

<template>
  <div class="graph-view">
    <h2 class="graph-title">{{ t('graph.title') }}</h2>
    <p v-if="graphNotes.length === 0" class="graph-empty">{{ t('graph.empty') }}</p>
    <svg v-else class="graph-svg" :viewBox="`0 0 ${WIDTH} ${HEIGHT}`" preserveAspectRatio="xMidYMid meet">
      <line
        v-for="(edge, i) in edges"
        :key="i"
        class="graph-edge"
        :x1="positionById.get(edge.source)?.x"
        :y1="positionById.get(edge.source)?.y"
        :x2="positionById.get(edge.target)?.x"
        :y2="positionById.get(edge.target)?.y"
      />
      <g v-for="node in nodes" :key="node.id" class="graph-node" @click="emit('selectNote', node.id)">
        <circle :cx="node.x" :cy="node.y" :r="radiusFor(node.id)" />
        <text :x="node.x" :y="node.y + radiusFor(node.id) + 14">{{ node.title }}</text>
      </g>
    </svg>
  </div>
</template>

<style scoped>
.graph-view {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-editor);
  overflow: auto;
  padding: 16px 24px;
  box-sizing: border-box;
}

.graph-title {
  flex: 0 0 auto;
  font-size: 20px;
  font-weight: 700;
  margin: 0 0 12px;
  color: var(--text-primary);
}

.graph-empty {
  color: var(--text-tertiary);
  text-align: center;
  margin-top: 60px;
}

.graph-svg {
  flex: 1 1 auto;
  min-height: 0;
  width: 100%;
}

.graph-edge {
  stroke: var(--border);
  stroke-width: 1.2;
  /* The layout publishes a new position every animation frame while it
     settles (see startLayout) - transitioning between them turns that
     into a smooth drift instead of a jittery, frame-stepped jump. */
  transition: x1 0.1s linear, y1 0.1s linear, x2 0.1s linear, y2 0.1s linear;
}

.graph-node {
  cursor: pointer;
}

.graph-node circle {
  fill: var(--accent-blue);
  stroke: var(--bg-editor);
  stroke-width: 2;
  transition: cx 0.1s linear, cy 0.1s linear, fill 0.15s ease;
}

.graph-node:hover circle {
  fill: var(--accent-blue-text);
}

.graph-node text {
  fill: var(--text-secondary);
  font-size: 11px;
  text-anchor: middle;
  pointer-events: none;
  user-select: none;
  transition: x 0.1s linear, y 0.1s linear;
}

.graph-node:hover text {
  fill: var(--text-primary);
}
</style>
