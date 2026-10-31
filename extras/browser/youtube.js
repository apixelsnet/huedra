// Huedra for YouTube. Runs in the page so it can read the player's data.
//
// While the open video has an HDR format (BT.2020 primaries with a PQ or HLG
// transfer), the tab title starts with MARKER. Huedra reads the browser's
// window title and turns HDR on. The formats offered are tested, not the one
// playing: YouTube plays HDR streams only on a display already in HDR.
// When the display turns HDR on, the video is loaded again at the same time,
// so the player picks an HDR stream.
//
// Makes no network requests and stores nothing.
// Feature doc: docs/features/05-video.md.

(() => {
  "use strict";

  // Must match MARKER in crates/huedra-core/src/video.rs.
  const MARKER = "⟦HDR⟧";
  const PREFIX = MARKER + " ";
  const HDR_TRANSFERS = new Set([
    "COLOR_TRANSFER_CHARACTERISTICS_SMPTEST2084",
    "COLOR_TRANSFER_CHARACTERISTICS_ARIB_STD_B67",
  ]);

  let hdr = false;

  function player() {
    return document.getElementById("movie_player");
  }

  function currentVideoId() {
    if (location.pathname !== "/watch") return null;
    return new URLSearchParams(location.search).get("v");
  }

  // The player's data for the video in the address bar, or null while it
  // still shows the previous one.
  function playerResponse(id) {
    const p = player();
    let response = null;
    try {
      if (p && typeof p.getPlayerResponse === "function") {
        response = p.getPlayerResponse();
      }
    } catch {
      response = null;
    }
    response = response || window.ytInitialPlayerResponse || null;
    const details = response && response.videoDetails;
    return details && details.videoId === id ? response : null;
  }

  function isHdrFormat(format) {
    const color = format && format.colorInfo;
    return (
      !!color &&
      color.primaries === "COLOR_PRIMARIES_BT2020" &&
      HDR_TRANSFERS.has(color.transferCharacteristics)
    );
  }

  function videoHasHdr() {
    const id = currentVideoId();
    if (!id) return false;
    const response = playerResponse(id);
    const formats = (response && response.streamingData && response.streamingData.adaptiveFormats) || [];
    return formats.some(isHdrFormat);
  }

  function applyTitle() {
    const title = document.title;
    const marked = title.startsWith(PREFIX);
    if (hdr && !marked) {
      document.title = PREFIX + title;
    } else if (!hdr && marked) {
      document.title = title.slice(PREFIX.length);
    }
  }

  function update() {
    hdr = videoHasHdr();
    applyTitle();
  }

  // Loads the same video again at the same time, so that the player chooses
  // its formats for the display as it is now.
  function reloadForHdr() {
    const p = player();
    const id = currentVideoId();
    const video = document.querySelector("video");
    if (!hdr || !p || !id || !video || typeof p.loadVideoById !== "function") return;
    const at = typeof p.getCurrentTime === "function" ? p.getCurrentTime() : video.currentTime;
    if (video.paused) {
      p.cueVideoById(id, at);
    } else {
      p.loadVideoById(id, at);
    }
  }

  // YouTube changes pages without reloading and rewrites the title often.
  document.addEventListener("yt-navigate-finish", update);
  document.addEventListener("yt-page-data-updated", update);
  // Media events do not bubble; capture them as each new video loads.
  document.addEventListener("loadedmetadata", update, true);
  new MutationObserver(applyTitle).observe(document.head, {
    subtree: true,
    childList: true,
    characterData: true,
  });

  const highRange = window.matchMedia("(dynamic-range: high)");
  highRange.addEventListener("change", () => {
    if (highRange.matches) reloadForHdr();
  });

  update();
})();
