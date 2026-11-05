import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Layer = { type: string; source: string; x: number; y: number; width: number; height: number; opacity: number; fit?: string | null; muted: boolean };
type RuntimeConfig = {
  id: string; title: string; kind: string; path: string; url?: string | null; scene?: { name: string; layers: Layer[] } | null;
  fps_limit: number; paused: boolean; volume: number; muted: boolean; slideshow_seconds: number;
};

declare global { interface Window { __OPEN_WALLPAPER__?: RuntimeConfig } }
const config = window.__OPEN_WALLPAPER__;
const root = document.querySelector<HTMLElement>("#wallpaper-root")!;
let paused = false;

function safeSource(path: string): string {
  if (/^https?:\/\//i.test(path)) return path;
  return convertFileSrc(path);
}

function mountWeb(): void {
  const frame = document.createElement("iframe");
  frame.className = "web-wallpaper";
  frame.allow = "autoplay; fullscreen; picture-in-picture";
  frame.referrerPolicy = "no-referrer";
  frame.src = config!.url || safeSource(config!.path);
  root.append(frame);
}

function mountScene(): void {
  const scene = config?.scene;
  if (!scene) return;
  for (const layer of scene.layers) {
    const element = layer.type === "video" ? document.createElement("video")
      : layer.type === "web" ? document.createElement("iframe")
      : document.createElement("img");
    element.className = "scene-layer";
    element.style.left = `${layer.x}%`;
    element.style.top = `${layer.y}%`;
    element.style.width = `${layer.width}%`;
    element.style.height = `${layer.height}%`;
    element.style.opacity = String(Math.max(0, Math.min(1, layer.opacity)));
    element.style.objectFit = layer.fit || "cover";
    if (element instanceof HTMLVideoElement) {
      element.src = safeSource(layer.source);
      element.autoplay = true;
      element.loop = true;
      element.dataset.sceneMuted = String(layer.muted);
      element.muted = layer.muted || config!.muted;
      element.volume = Math.max(0, Math.min(1, config!.volume / 100));
      element.playsInline = true;
      element.preload = "auto";
      element.style.pointerEvents = "none";
    } else if (element instanceof HTMLImageElement) {
      element.src = safeSource(layer.source);
      element.alt = "";
      element.draggable = false;
    } else {
      element.src = safeSource(layer.source);
      element.setAttribute("allow", "autoplay; fullscreen");
      element.setAttribute("sandbox", "allow-scripts allow-same-origin");
    }
    root.append(element);
  }
}

function applyPause(value: boolean): void {
  paused = value;
  document.documentElement.dataset.paused = String(value);
  document.querySelectorAll<HTMLVideoElement>("video").forEach((video) => {
    if (value) video.pause(); else void video.play().catch(() => undefined);
  });
}

async function start(): Promise<void> {
  if (!config) { document.body.style.background = "#000"; return; }
  document.title = config.title;
  if (config.kind === "Web") mountWeb();
  else if (config.kind === "Scene") mountScene();
  // Video, GIF, still-image and slideshow surfaces are rendered by libmpv into this HWND.
  if (config.paused) applyPause(true);
  await listen<boolean>("ow:pause", (event) => applyPause(event.payload));
  await listen<{ volume: number; muted: boolean }>("ow:audio-settings", (event) => {
    document.querySelectorAll<HTMLVideoElement>("video").forEach((video) => {
      video.volume = Math.max(0, Math.min(1, event.payload.volume / 100));
      video.muted = video.dataset.sceneMuted === "true" || event.payload.muted;
    });
  });
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) document.querySelectorAll<HTMLVideoElement>("video").forEach((video) => video.pause());
    else if (!paused) document.querySelectorAll<HTMLVideoElement>("video").forEach((video) => void video.play().catch(() => undefined));
  });
}

void start();
