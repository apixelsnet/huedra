// Huedra for streaming sites whose streams are protected, so whether a video
// is in HDR cannot be read.
//
// While a video is full screen, the tab title starts with the site's marker,
// "⟦Video netflix⟧". Huedra counts it as HDR video only for the sites turned
// on in its settings, and only while the browser window is full screen.
//
// Runs in the extension's own context, not the page's. Makes no network
// requests and stores nothing.
// Feature doc: docs/features/05-video.md.

(() => {
  "use strict";

  // Keys must match StreamingSite::key in crates/huedra-core/src/video.rs;
  // the hosts must match the manifest.
  const SITES = {
    "www.netflix.com": "netflix",
    "www.disneyplus.com": "disney-plus",
    "www.primevideo.com": "prime-video",
    "play.hbomax.com": "hbo-max",
    "play.max.com": "hbo-max",
    "tv.apple.com": "apple-tv",
  };
  const site = SITES[location.hostname];
  if (!site) return;
  // Must match SITE_MARKER_START in crates/huedra-core/src/video.rs.
  const PREFIX = "⟦Video " + site + "⟧ ";

  let marked = false;

  function isReadyVideo(element) {
    return element.localName === "video" && element.readyState >= 1;
  }

  // Whether the element, its descendants or their open shadow trees hold a
  // video with its size known.
  function holdsVideo(root) {
    if (isReadyVideo(root)) return true;
    if (root.shadowRoot && holdsVideo(root.shadowRoot)) return true;
    for (const element of root.querySelectorAll("*")) {
      if (isReadyVideo(element)) return true;
      if (element.shadowRoot && holdsVideo(element.shadowRoot)) return true;
    }
    return false;
  }

  function applyTitle() {
    const title = document.title;
    const has = title.startsWith(PREFIX);
    if (marked && !has) {
      document.title = PREFIX + title;
    } else if (!marked && has) {
      document.title = title.slice(PREFIX.length);
    }
  }

  function update() {
    const full = document.fullscreenElement;
    marked = !!full && holdsVideo(full);
    applyTitle();
  }

  document.addEventListener("fullscreenchange", update);
  // Media events do not bubble; capture them as a video loads or is cleared
  // while the page is full screen.
  document.addEventListener("loadedmetadata", update, true);
  document.addEventListener("emptied", update, true);
  // The sites rewrite the title as pages change.
  new MutationObserver(applyTitle).observe(document.head, {
    subtree: true,
    childList: true,
    characterData: true,
  });

  update();
})();
