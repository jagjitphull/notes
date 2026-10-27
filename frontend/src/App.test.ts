import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";

const { listenMock, apiMocks } = vi.hoisted(() => {
  const apiMocks = {
    getNotesRoot: vi.fn(),
    vaultStatus: vi.fn(),
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
const { i18n } = await import("./i18n");

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
    apiMocks.vaultStatus.mockResolvedValue({ encrypted: false, unlocked: true });
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
        plugins: [i18n],
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

// Regression coverage for a bug found in manual QA: the FOLDERS list in the
// sidebar is flat (no indentation or grouping by parent), so a new folder
// could be created with the exact same name as one already in that list -
// including the unrenameable root entry, labeled "Notes" - leaving two
// entries a user could only tell apart by which one happened to be
// selected. Fixed by validating the name against every existing folder
// before calling the backend, which itself only dedupes by directory path
// and would happily create a second same-named directory.
describe("App.vue duplicate folder names", () => {
  // PromptModal and ContextMenu both render via <Teleport to="body">, which
  // moves their DOM out from under wrapper.element - wrapper.find() only
  // searches its own subtree, so teleported content has to be queried from
  // document.body directly instead.
  const body = () => new DOMWrapper(document.body);

  let activeWrapper: Awaited<ReturnType<typeof mountReady>> | null = null;

  beforeEach(() => {
    for (const fn of Object.values(apiMocks)) fn.mockReset();
    apiMocks.getNotesRoot.mockResolvedValue("/tmp/notes");
    apiMocks.vaultStatus.mockResolvedValue({ encrypted: false, unlocked: true });
    apiMocks.listFolders.mockResolvedValue([
      { id: "", name: "Notes", parentId: null, color: null },
      { id: "work", name: "Work", parentId: "", color: null },
    ]);
    apiMocks.listNotes.mockResolvedValue([]);
    apiMocks.listTags.mockResolvedValue([]);
    apiMocks.smartSearchAvailable.mockResolvedValue(true);
    apiMocks.searchNotes.mockResolvedValue([]);
  });

  afterEach(() => {
    // Unmounting tears down any still-teleported modal/menu content too, so
    // it doesn't leak into the next test (tests in a file share one jsdom
    // document).
    activeWrapper?.unmount();
    activeWrapper = null;
  });

  async function mountReady() {
    const wrapper = mount(App, {
      global: {
        plugins: [i18n],
        stubs: { Editor: true, NoteList: true, FirstRunSetup: true },
      },
    });
    await flushPromises(); // onMounted: getNotesRoot + refreshData + availability check
    activeWrapper = wrapper;
    return wrapper;
  }

  async function openNewFolderPrompt(wrapper: Awaited<ReturnType<typeof mountReady>>) {
    await wrapper.find('button[aria-label="New Folder"]').trigger("click");
  }

  async function openRenameFolderPrompt(wrapper: Awaited<ReturnType<typeof mountReady>>) {
    const workButton = wrapper.findAll(".nav-item").find((b) => b.text().includes("Work"));
    await workButton!.trigger("contextmenu");
    const renameItem = body()
      .findAll("button.menu-item")
      .find((b) => b.text().includes("Rename"));
    await renameItem!.trigger("click");
  }

  it("rejects a new folder name colliding with an existing one, case- and whitespace-insensitively", async () => {
    const wrapper = await mountReady();
    await openNewFolderPrompt(wrapper);

    await body().find(".modal input").setValue("  work  ");
    await body().find(".modal").trigger("submit");

    expect(body().find(".modal-error").text()).toContain("work");
    expect(apiMocks.createFolder).not.toHaveBeenCalled();
    expect(body().find(".modal").exists()).toBe(true); // stays open for a correction
  });

  it("rejects a name colliding with the root Notes entry", async () => {
    const wrapper = await mountReady();
    await openNewFolderPrompt(wrapper);

    await body().find(".modal input").setValue("Notes");
    await body().find(".modal").trigger("submit");

    expect(body().find(".modal-error").exists()).toBe(true);
    expect(apiMocks.createFolder).not.toHaveBeenCalled();
  });

  it("allows a unique new folder name through and closes the prompt", async () => {
    apiMocks.createFolder.mockResolvedValue("personal");
    const wrapper = await mountReady();
    await openNewFolderPrompt(wrapper);

    await body().find(".modal input").setValue("Personal");
    await body().find(".modal").trigger("submit");
    await flushPromises();

    expect(apiMocks.createFolder).toHaveBeenCalledWith("", "Personal");
    expect(body().find(".modal").exists()).toBe(false);
  });

  it("clears the error once the user edits the name", async () => {
    const wrapper = await mountReady();
    await openNewFolderPrompt(wrapper);

    const input = body().find(".modal input");
    await input.setValue("Work");
    await body().find(".modal").trigger("submit");
    expect(body().find(".modal-error").exists()).toBe(true);

    await input.setValue("Work ");
    expect(body().find(".modal-error").exists()).toBe(false);
  });

  it("doesn't block renaming a folder to the name it already has", async () => {
    apiMocks.renameFolder.mockResolvedValue("work");
    const wrapper = await mountReady();
    await openRenameFolderPrompt(wrapper);

    await body().find(".modal").trigger("submit"); // initial value is unchanged
    await flushPromises();

    expect(apiMocks.renameFolder).toHaveBeenCalledWith("work", "Work");
  });

  it("rejects renaming a folder to a name another folder already has", async () => {
    const wrapper = await mountReady();
    await openRenameFolderPrompt(wrapper);

    await body().find(".modal input").setValue("Notes");
    await body().find(".modal").trigger("submit");

    expect(body().find(".modal-error").exists()).toBe(true);
    expect(apiMocks.renameFolder).not.toHaveBeenCalled();
  });
});
