// The loading screen (index.html's #loading): trunk's initializer for the game's wasm (`data-initializer` on the
// game's rust link; trunk 0.21). Three phases: the download with a progress bar, "starting" from the compiled wasm to
// the game's first frames (the game dispatches `game-ready` on window then: game's main.rs), and a failure message.
// Without WebGL2 it stops before the game's script fetches the wasm (the page's preload link may still fetch it). The lab's worker (the second wasm binary) has no initializer.

const MB = 1e6;

const el = (id) => document.getElementById(id);

function phase(name) {
  el("loading").dataset.phase = name;
}

function status(label, amount) {
  el("load-label").textContent = label;
  el("load-amount").textContent = amount ?? "";
}

function fail(title, text, detail) {
  phase("failed");
  el("load-error-title").textContent = title;
  el("load-error-text").textContent = text;
  el("load-error-detail").textContent = detail ? String(detail) : "";
}

function hasWebGl2() {
  try {
    return !!document.createElement("canvas").getContext("webgl2");
  } catch {
    return false;
  }
}

// An error while the game starts (a panic before its first frame) would leave "starting" up for ever.
function onStartupError(event) {
  if (el("loading")?.dataset.phase !== "starting") return;
  const reason = event.reason ?? event.error ?? event.message;
  fail("The game stopped while starting", "Reloading the page may help. If it happens again, the browser's console says more.", reason);
}

// From the last byte: compiling, instantiating, then main (the save is read from IndexedDB, the app is built) until
// the game's first frames.
function starting() {
  const loading = el("loading");
  if (!loading || loading.dataset.phase !== "download") return;
  phase("starting");
  loading.classList.remove("indeterminate");
  el("load-fill").style.transform = "";
  status("Starting");
  addEventListener("error", onStartupError);
  addEventListener("unhandledrejection", onStartupError);
}

function ready() {
  removeEventListener("error", onStartupError);
  removeEventListener("unhandledrejection", onStartupError);
  const loading = el("loading");
  if (!loading || loading.dataset.phase === "failed") return;
  phase("done");
  // After the fade (index.html's CSS); the canvas keeps the focus the page gave it.
  setTimeout(() => loading.remove(), 600);
}

export default function initializer() {
  if (!hasWebGl2()) {
    fail(
      "This browser can't show the game",
      "The game draws with WebGL2, which this browser doesn't offer or has turned off. A current Chrome, Firefox, " +
        "Edge or Safari with hardware acceleration on can run it.",
    );
    // Stops trunk's loader before it fetches and starts the wasm.
    throw new Error("WebGL2 is not available");
  }
  addEventListener("game-ready", ready, { once: true });
  return {
    onStart() {
      phase("download");
      status("Downloading");
    },
    // `total` is the served wasm's size (uncompressed; written into the page by Trunk.toml's hook), `current` counts
    // the bytes after the browser has unpacked them, so the two match even though GitHub Pages sends gzip. Without a
    // total the bar is indeterminate.
    onProgress({ current, total }) {
      // Trunk says 100 % once more after the last byte.
      if (el("loading")?.dataset.phase !== "download") return;
      const known = Number.isFinite(total) && total > 0;
      el("loading").classList.toggle("indeterminate", !known);
      if (known) {
        const share = Math.min(current / total, 1);
        el("load-fill").style.transform = `scaleX(${share})`;
        status("Downloading", `${(current / MB).toFixed(1)} / ${(total / MB).toFixed(1)} MB`);
        if (current >= total) starting();
      } else {
        status("Downloading", `${(current / MB).toFixed(1)} MB`);
      }
    },
    // Without a total, the download ends here.
    onSuccess: starting,
    onFailure(reason) {
      fail(
        "The game could not load",
        "Its files didn't arrive or the browser couldn't compile them. Check the connection and reload the page.",
        reason,
      );
    },
  };
}
