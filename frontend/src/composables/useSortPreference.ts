import { ref } from "vue";

export type SortField = "updatedAt" | "createdAt" | "title";
export type SortDirection = "asc" | "desc";

const STORAGE_KEY_FIELD = "notes-sort-field";
const STORAGE_KEY_DIRECTION = "notes-sort-direction";

const field = ref<SortField>(readStoredField());
const direction = ref<SortDirection>(readStoredDirection());

function readStoredField(): SortField {
  const stored = localStorage.getItem(STORAGE_KEY_FIELD);
  return stored === "createdAt" || stored === "title" || stored === "updatedAt"
    ? stored
    : "updatedAt";
}

function readStoredDirection(): SortDirection {
  return localStorage.getItem(STORAGE_KEY_DIRECTION) === "asc" ? "asc" : "desc";
}

/**
 * Note-list sort order (field + direction), persisted across restarts.
 * Default (updatedAt, desc) matches the app's original fixed sort, so
 * nobody's list visibly reorders itself just from this shipping.
 */
export function useSortPreference() {
  function setField(next: SortField) {
    field.value = next;
    localStorage.setItem(STORAGE_KEY_FIELD, next);
  }

  function setDirection(next: SortDirection) {
    direction.value = next;
    localStorage.setItem(STORAGE_KEY_DIRECTION, next);
  }

  function toggleDirection() {
    setDirection(direction.value === "asc" ? "desc" : "asc");
  }

  return { field, direction, setField, setDirection, toggleDirection };
}
