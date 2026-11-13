window.addEventListener("message", (event) => {
  if (event.source !== window.parent || event.data?.type !== "openwallpaper:pause") return;
  document.documentElement.dataset.openwallpaperPaused = String(Boolean(event.data.paused));
});

