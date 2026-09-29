import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";

const { listenMock, apiMocks } = vi.hoisted(() => {
  const apiMocks = {
    getNotesRoot: vi.fn(),
    listFolders: vi.fn(),
    listNotes: vi.fn(),
    listTags: vi.fn(),
    searchNotes: vi.fn(),
    smartSearch: vi.fn(),
    smartSearchAvailable: vi.fn(),
    createNote: vi.fn(),
    createFolder: vi.fn(),
    renameFolder: vi.fn(),
    deleteFolder: vi.fn(),
    deleteNotePermanently: vi.fn(),
    moveNote: vi.fn(),
    saveNoteBody: vi.fn(),
    getNoteBody: vi.fn(),
    setNotePinned: vi.fn(),
    setNoteDeleted: vi.fn(),
    addNoteTag: vi.fn(),
    removeNoteTag: vi.fn(),
  };
  return { listenMock: vi.fn().mockResolvedValue(() => {}), apiMocks };
});

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("./api", () => apiMocks);

const App = (await import("./App.vue")).default;
const Sidebar = (await import("./components/Sidebar.vue")).default;

// Regression coverage for a real bug found in manual testing: Smart
// Search's availability was only ever checked once at launch, so once
// Ollama went down (or was never up at launch), the toggle stayed hidden
// for the rest of the session even after Ollama came back - fixed by
// polling. These tests pin the two behaviors that made that a real fix:
// a failed search falls back to regular search immediately, and it
// doesn't wait for a search to notice availability changed.
describe("App.vue Smart Search", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    for (const fn of Object.values(apiMocks)) fn.mockReset();
    apiMocks.getNotesRoot.mockResolvedValue("/tmp/notes");
    apiMocks.listFolders.mockResolvedValue([]);
    apiMocks.listNotes.mockResolvedValue([]);
    apiMocks.listTags.mockResolvedValue([]);
    apiMocks.smartSearchAvailable.mockResolvedValue(true);
    apiMocks.searchNotes.mockResolvedValue([]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  async function mountReady() {
    const wrapper = mount(App, {
      global: {
        stubs: {
          Editor: true,
          NoteList: true,
          FirstRunSetup: true,
          ContextMenu: true,
          PromptModal: true,
        },
      },
    });
    await flushPromises(); // onMounted: getNotesRoot + refreshData + availability check
    return wrapper;
  }

  it("shows the Smart Search toggle once Ollama is reachable", async () => {
    const wrapper = await mountReady();
    expect(wrapper.findComponent(Sidebar).props("smartSearchSupported")).toBe(true);
  });

  it("hides the toggle from the start when Ollama isn't reachable", async () => {
    apiMocks.smartSearchAvailable.mockResolvedValue(false);
    const wrapper = await mountReady();
    expect(wrapper.findComponent(Sidebar).props("smartSearchSupported")).toBe(false);
  });

  it("falls back to regular search and hides the toggle when a smart search fails", async () => {
    apiMocks.smartSearch.mockRejectedValue(new Error("Ollama unreachable"));
    const wrapper = await mountReady();

    const searchInput = wrapper.find('input[aria-label="Search notes"]');
    const smartToggle = wrapper.find("button.smart-search-toggle");
    expect(smartToggle.exists()).toBe(true);

    await smartToggle.trigger("click"); // enable smart mode
    await searchInput.setValue("flight hotel");
    await vi.advanceTimersByTimeAsync(500); // past the 400ms smart-search debounce
    await flushPromises();

    expect(apiMocks.smartSearch).toHaveBeenCalledWith("flight hotel");
    expect(apiMocks.searchNotes).toHaveBeenCalledWith("flight hotel");
    expect(wrapper.find("button.smart-search-toggle").exists()).toBe(false);
  });

  it("polls availability so the toggle returns on its own after Ollama comes back", async () => {
    apiMocks.smartSearchAvailable.mockResolvedValue(false);
    const wrapper = await mountReady();
    expect(wrapper.findComponent(Sidebar).props("smartSearchSupported")).toBe(false);

    apiMocks.smartSearchAvailable.mockResolvedValue(true);
    await vi.advanceTimersByTimeAsync(15000); // the availability poll interval
    await flushPromises();

    expect(wrapper.findComponent(Sidebar).props("smartSearchSupported")).toBe(true);
  });
});
