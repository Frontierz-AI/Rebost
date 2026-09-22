// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatImage, VisionLimits } from "./api";

const mock = vi.hoisted(() => ({
  app: {
    engine: {
      vision: { maxImages: 2, maxEdge: 1024, tokensPerImage: 1024 } as VisionLimits | null,
    },
  },
  chatState: {
    navigation: 0,
    activeThreadId: "thread-a" as string | null,
    imports: {} as Record<string, number>,
    imageImports: {} as Record<string, number>,
    imageDrafts: {} as Record<string, ChatImage[]>,
  },
  add: vi.fn(),
  remove: vi.fn(),
  notify: vi.fn(),
  error: vi.fn(),
  ensure: vi.fn(),
}));
vi.mock("./api", () => ({ api: { chatImageAdd: mock.add, chatImageRemove: mock.remove } }));
vi.mock("./stores.svelte", () => ({
  app: mock.app,
  chatState: mock.chatState,
  ensureActiveThread: mock.ensure,
  notify: mock.notify,
  notifyInvokeError: mock.error,
}));
vi.mock("./i18n.svelte", () => ({ t: (key: string) => key }));
import {
  addChatImages,
  isImagePath,
  pastedImages,
  removeChatImage,
  MAX_IMAGE_BYTES,
} from "./chat-images";

const image: ChatImage = { id: "img_a", name: "screen.png", width: 100, height: 100, bytes: 100 };

beforeEach(() => {
  vi.clearAllMocks();
  mock.app.engine.vision = { maxImages: 2, maxEdge: 1024, tokensPerImage: 1024 };
  mock.chatState.navigation = 0;
  mock.chatState.activeThreadId = "thread-a";
  mock.chatState.imports = {};
  mock.chatState.imageImports = {};
  mock.chatState.imageDrafts = {};
  mock.add.mockResolvedValue(image);
  mock.remove.mockResolvedValue(undefined);
  mock.ensure.mockResolvedValue("new-thread");
});

describe("chat image intake", () => {
  it("does not route images through the document importer", () => {
    expect(isImagePath("C:\\photos\\SHOT.JPEG")).toBe(true);
    expect(isImagePath("/folder/shot.png.pdf")).toBe(false);
    expect(isImagePath("picture.webp")).toBe(true);
  });
  it("keeps a paste bound to the conversation where it began", async () => {
    let complete!: (value: ChatImage) => void;
    mock.add.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const work = addChatImages(["/screen.png"]);
    await vi.waitFor(() => expect(mock.add).toHaveBeenCalled());
    expect(mock.chatState.imageImports["thread-a"]).toBe(1);
    mock.chatState.activeThreadId = "thread-b";
    mock.chatState.navigation++;
    complete(image);
    await work;
    expect(mock.chatState.imageDrafts["thread-a"]).toEqual([image]);
    expect(mock.chatState.imageDrafts["thread-b"]).toBeUndefined();
    expect(mock.chatState.imageImports["thread-a"]).toBe(0);
  });
  it("serializes simultaneous pastes so they cannot exceed the image count", async () => {
    mock.app.engine.vision!.maxImages = 1;
    await Promise.all([addChatImages(["one.png"]), addChatImages(["two.png"])]);
    expect(mock.add).toHaveBeenCalledTimes(1);
    expect(mock.notify).toHaveBeenCalledWith("images.limit");
    expect(mock.chatState.imports["thread-a"]).toBe(0);
    expect(mock.chatState.imageImports["thread-a"]).toBe(0);
  });
  it("keeps the count limit when the first paste creates the conversation", async () => {
    mock.app.engine.vision!.maxImages = 1;
    mock.chatState.activeThreadId = null;
    let finish!: (value: ChatImage) => void;
    mock.ensure.mockImplementationOnce(async () => {
      mock.chatState.activeThreadId = "new-thread";
      return "new-thread";
    });
    mock.add.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const first = addChatImages(["one.png"]);
    await vi.waitFor(() => expect(mock.add).toHaveBeenCalled());
    const second = addChatImages(["two.png"]);
    finish(image);
    await Promise.all([first, second]);
    expect(mock.add).toHaveBeenCalledTimes(1);
    expect(mock.chatState.imageDrafts["new-thread"]).toEqual([image]);
  });
  it("rejects images on a text-only model without touching draft state", async () => {
    mock.app.engine.vision = null;
    await addChatImages(["screen.png"]);
    expect(mock.add).not.toHaveBeenCalled();
    expect(mock.notify).toHaveBeenCalledWith("images.unavailable");
  });
  it("passes pasted binary data to the same backend intake", async () => {
    await addChatImages([new File(["png"], "clipboard.png", { type: "image/png" })]);
    expect(mock.add).toHaveBeenCalledWith("thread-a", "images.pasted", { data: "cG5n" });
  });
  it("rejects oversize clipboard content before invoking Rust", async () => {
    const file = new File(["png"], "large.png", { type: "image/png" });
    Object.defineProperty(file, "size", { value: MAX_IMAGE_BYTES + 1 });
    await addChatImages([file]);
    expect(mock.add).not.toHaveBeenCalled();
    expect(mock.error).toHaveBeenCalled();
    expect(mock.chatState.imports["thread-a"]).toBe(0);
    expect(mock.chatState.imageImports["thread-a"]).toBe(0);
  });
  it("finds image clipboard items without treating text as an image", () => {
    const file = new File(["png"], "image.png", { type: "image/png" });
    const data = {
      items: [
        { kind: "string", type: "text/plain", getAsFile: () => null },
        { kind: "file", type: "image/png", getAsFile: () => file },
      ],
    } as unknown as DataTransfer;
    expect(pastedImages(data)).toEqual([file]);
    expect(pastedImages(null)).toEqual([]);
  });
  it("removes only the requested draft image", () => {
    mock.chatState.imageDrafts["thread-a"] = [image, { ...image, id: "img_b" }];
    removeChatImage("thread-a", image);
    expect(mock.chatState.imageDrafts["thread-a"].map((item) => item.id)).toEqual(["img_b"]);
    expect(mock.remove).toHaveBeenCalledWith("thread-a", "img_a");
  });
});
