import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles/main.css";

type WallpaperKind = "Video" | "Gif" | "Image" | "Web" | "Slideshow" | "Scene";
type Wallpaper = {
  id: string; title: string; kind: WallpaperKind; path: string; preview_data_uri?: string | null; preview_path?: string | null;
  source: string; favorite: boolean; tags: string[]; description?: string | null; file_count: number;
};
type Monitor = { id: string; name: string; x: number; y: number; width: number; height: number; scale_factor: number; primary: boolean };
type Settings = {
  autostart: boolean; start_minimized: boolean; pause_in_games: boolean; pause_on_lock: boolean;
  pause_on_battery: boolean; fps_limit: number; volume: number; muted: boolean; slideshow_seconds: number;
  game_processes: string[]; monitor_wallpapers: Record<string, string>; monitor_fps: Record<string, number>;
};
type Collection = { id: string; name: string; wallpaper_ids: string[] };
type WorkshopItem = { published_file_id: string; title: string; preview_url?: string | null; item_url: string; installed: boolean };

type Page = "library" | "workshop" | "collections" | "settings";
const state: {
  page: Page; wallpapers: Wallpaper[]; monitors: Monitor[]; settings: Settings | null; collections: Collection[];
  workshop: WorkshopItem[]; selectedId: string | null; selectedMonitor: string; search: string; filter: string;
  favoritesOnly: boolean; workshopQuery: string; paused: boolean; toast: string; activeCollection: string | null;
} = {
  page: "library", wallpapers: [], monitors: [], settings: null, collections: [], workshop: [], selectedId: null,
  selectedMonitor: "all", search: "", filter: "all", favoritesOnly: false, workshopQuery: "", paused: false, toast: "", activeCollection: null,
};

const app = document.querySelector<HTMLDivElement>("#app")!;
const kindLabel: Record<WallpaperKind, string> = { Video: "Видео", Gif: "GIF", Image: "Изображение", Web: "Веб", Slideshow: "Слайд-шоу", Scene: "Сцена" };

function esc(value: unknown): string {
  return String(value ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));
}
function selectedWallpaper(): Wallpaper | undefined { return state.wallpapers.find((item) => item.id === state.selectedId); }
function notify(message: string): void {
  state.toast = message;
  renderToast();
  window.setTimeout(() => { if (state.toast === message) { state.toast = ""; renderToast(); } }, 2800);
}
function renderToast(): void {
  let node = document.querySelector<HTMLElement>(".toast");
  if (!node) { node = document.createElement("div"); node.className = "toast"; document.body.append(node); }
  node.textContent = state.toast;
  node.classList.toggle("visible", Boolean(state.toast));
}

async function refresh(): Promise<void> {
  const [wallpapers, monitors, settings, collections] = await Promise.all([
    invoke<Wallpaper[]>("get_library", { query: "", kind: "all", favoritesOnly: false }),
    invoke<Monitor[]>("get_monitors"),
    invoke<Settings>("get_settings"),
    invoke<Collection[]>("get_collections"),
  ]);
  state.wallpapers = wallpapers;
  state.monitors = monitors;
  state.settings = settings;
  state.collections = collections;
  if (!state.selectedId || !wallpapers.some((w) => w.id === state.selectedId)) state.selectedId = wallpapers[0]?.id ?? null;
  if (state.selectedMonitor !== "all" && !monitors.some((m) => m.id === state.selectedMonitor)) state.selectedMonitor = "all";
  render();
}

function shell(content: string): string {
  return `
    <div class="app-shell">
      <aside class="sidebar">
        <div class="brand"><div class="brand-mark"><span></span><i></i></div><div><b>OpenWallpaper</b><small>DESKTOP STUDIO</small></div></div>
        <div class="nav-label">РАБОЧЕЕ ПРОСТРАНСТВО</div>
        <nav class="nav">
          <button data-page="library" class="nav-item ${state.page === "library" ? "active" : ""}"><span class="nav-glyph">▦</span> Библиотека <span class="nav-count">${state.wallpapers.length}</span></button>
          <button data-page="workshop" class="nav-item ${state.page === "workshop" ? "active" : ""}"><span class="nav-glyph">◈</span> Steam Workshop</button>
          <button data-page="collections" class="nav-item ${state.page === "collections" ? "active" : ""}"><span class="nav-glyph">▤</span> Подборки <span class="nav-count">${state.collections.length}</span></button>
        </nav>
        <div class="sidebar-spacer"></div>
        <div class="sidebar-system">
          <div class="system-dot ${state.paused ? "paused" : ""}"></div>
          <div><b>${state.paused ? "Воспроизведение на паузе" : "Система активна"}</b><small>${state.monitors.length} монитора · Windows</small></div>
        </div>
        <button data-page="settings" class="nav-item settings-link ${state.page === "settings" ? "active" : ""}"><span class="nav-glyph">⚙</span> Настройки</button>
        <div class="sidebar-version">OPEN SOURCE · v0.1.0</div>
      </aside>
      <main class="main-area">
        <header class="topbar" data-tauri-drag-region>
          <div class="breadcrumbs" data-tauri-drag-region><span>OpenWallpaper</span><i>/</i><b>${({ library: "Библиотека", workshop: "Steam Workshop", collections: "Подборки", settings: "Настройки" } as Record<Page, string>)[state.page]}</b></div>
          <div class="top-actions">
            <button class="icon-button ${state.paused ? "is-paused" : ""}" data-action="toggle-pause" title="Пауза / продолжить">${state.paused ? "▶" : "Ⅱ"}</button>
            <button class="window-control" data-action="minimize" title="Свернуть">−</button>
            <button class="window-control close" data-action="hide" title="Свернуть в трей">×</button>
          </div>
        </header>
        <div class="page-content">${content}</div>
      </main>
    </div>`;
}

function cardMarkup(item: Wallpaper): string {
  const previewSource = item.preview_path ? convertFileSrc(item.preview_path) : item.preview_data_uri;
  const preview = previewSource
    ? `<img src="${esc(previewSource)}" alt="" loading="lazy">`
    : item.kind === "Video" ? `<video class="card-video-preview" data-preview-video src="${esc(convertFileSrc(item.path))}" muted playsinline preload="none"></video>`
    : `<div class="cover-placeholder kind-${item.kind.toLowerCase()}"><div class="placeholder-icon">${({ Video: "▶", Gif: "GIF", Image: "▧", Web: "⌘", Slideshow: "▤", Scene: "✧" } as Record<WallpaperKind, string>)[item.kind]}</div><span>${kindLabel[item.kind]}</span></div>`;
  return `<article class="wallpaper-card ${state.selectedId === item.id ? "selected" : ""}" data-select="${esc(item.id)}">
    <div class="card-cover">${preview}<div class="cover-gradient"></div><span class="kind-chip">${kindLabel[item.kind]}</span>
      <button class="favorite-button ${item.favorite ? "favorited" : ""}" data-action="favorite" data-id="${esc(item.id)}" title="Избранное">${item.favorite ? "★" : "☆"}</button>
      <div class="card-hover"><span>Предпросмотр</span></div>
    </div>
    <div class="card-meta"><div class="card-title">${esc(item.title)}</div><div class="card-source">${esc(item.source)}${item.file_count > 1 ? ` · ${item.file_count} файлов` : ""}</div></div>
  </article>`;
}

function libraryPage(): string {
  const visible = state.wallpapers.filter((item) => {
    const q = state.search.toLowerCase();
    const qMatch = !q || item.title.toLowerCase().includes(q) || item.source.toLowerCase().includes(q) || item.tags.some((t) => t.toLowerCase().includes(q));
    const fMatch = state.filter === "all" || item.kind.toLowerCase() === state.filter.toLowerCase();
    const collectionMatch = !state.activeCollection || state.collections.find((c) => c.id === state.activeCollection)?.wallpaper_ids.includes(item.id);
    return qMatch && fMatch && (!state.favoritesOnly || item.favorite) && collectionMatch;
  });
  const selected = selectedWallpaper();
  return `<div class="page-heading">
    <div><div class="eyebrow">ВАША КОЛЛЕКЦИЯ</div><h1>Библиотека обоев</h1><p>Все ваши фоны в одном месте. Импортируйте файлы или выбирайте обои из Steam Workshop.</p></div>
    <div class="heading-actions"><button class="button button-quiet" data-action="add-web">＋ Добавить URL</button><button class="button button-primary" data-action="import">⇧ Импортировать папку</button></div>
  </div>
  <div class="stats-row">
    <div class="stat-card"><span>ВСЕ ОБОИ</span><b>${state.wallpapers.length}</b><small>в локальной библиотеке</small></div>
    <div class="stat-card"><span>ИЗБРАННОЕ</span><b>${state.wallpapers.filter((w) => w.favorite).length}</b><small>сохранённых обоев</small></div>
    <div class="stat-card"><span>МОНИТОРЫ</span><b>${state.monitors.length}</b><small>доступно для настройки</small></div>
    <div class="stat-card stat-status"><span>СОСТОЯНИЕ</span><b>${state.paused ? "Пауза" : "Активно"}</b><small>${state.settings?.fps_limit ?? 30} FPS · ${state.settings?.muted ? "без звука" : "звук включён"}</small></div>
  </div>
  <div class="library-toolbar">
    <div class="search-box"><span>⌕</span><input id="library-search" type="search" placeholder="Поиск по библиотеке..." value="${esc(state.search)}"></div>
    <select id="kind-filter" class="select-control"><option value="all">Все форматы</option>${Object.entries(kindLabel).map(([key, label]) => `<option value="${key.toLowerCase()}" ${state.filter === key.toLowerCase() ? "selected" : ""}>${label}</option>`).join("")}</select>
    <button class="filter-button ${state.favoritesOnly ? "active" : ""}" data-action="favorites">${state.favoritesOnly ? "★" : "☆"} Избранное</button>
    <button class="icon-button" data-action="rescan" title="Обновить библиотеку">↻</button>
  </div>
  <div class="library-layout">
    <section class="library-results">
      <div class="section-line"><b>${visible.length} обоев${state.activeCollection ? ` · ${esc(state.collections.find((c) => c.id === state.activeCollection)?.name ?? "Подборка")}` : ""}</b><span>${state.activeCollection ? `<button class="text-link" data-action="clear-collection">Показать всё</button>` : "Сортировка: название"}</span></div>
      ${visible.length ? `<div class="wallpaper-grid">${visible.map(cardMarkup).join("")}</div>` : `<div class="empty-state"><div class="empty-icon">▧</div><h3>Здесь пока пусто</h3><p>${state.wallpapers.length ? "Измените фильтр или поисковый запрос." : "Добавьте папку с видео, изображениями, GIF или веб-обоями."}</p><button class="button button-primary" data-action="import">Импортировать папку</button></div>`}
    </section>
    <aside class="detail-panel">${selected ? detailMarkup(selected) : `<div class="detail-empty">Выберите обои, чтобы увидеть подробности</div>`}</aside>
  </div>`;
}

function detailMarkup(item: Wallpaper): string {
  const previewSource = item.preview_path ? convertFileSrc(item.preview_path) : item.preview_data_uri;
  const cover = previewSource ? `<img src="${esc(previewSource)}" alt="">` : item.kind === "Video" ? `<video class="detail-video-preview" src="${esc(convertFileSrc(item.path))}" muted autoplay loop playsinline></video>` : `<div class="detail-placeholder">${kindLabel[item.kind]}</div>`;
  return `<div class="detail-cover">${cover}<span class="detail-cover-type">${kindLabel[item.kind]}</span></div>
    <div class="detail-body"><div class="eyebrow">ВЫБРАННЫЕ ОБОИ</div><h2>${esc(item.title)}</h2><p class="detail-description">${esc(item.description || "Локальный файл из вашей коллекции.")}</p>
      <div class="detail-facts"><div><span>ФОРМАТ</span><b>${kindLabel[item.kind]}</b></div><div><span>ИСТОЧНИК</span><b>${esc(item.source)}</b></div><div><span>ФАЙЛЫ</span><b>${item.file_count}</b></div></div>
      <label class="field-label" for="monitor-select">ПРИМЕНИТЬ НА МОНИТОР</label><select id="monitor-select" class="select-control select-full"><option value="all" ${state.selectedMonitor === "all" ? "selected" : ""}>Все мониторы</option>${state.monitors.map((m) => `<option value="${esc(m.id)}" ${state.selectedMonitor === m.id ? "selected" : ""}>${esc(m.name)}${m.primary ? " · основной" : ""} (${m.width}×${m.height})</option>`).join("")}</select>
      <button class="button button-primary apply-button" data-action="apply" data-id="${esc(item.id)}">Установить обои <span>→</span></button>
      <div class="detail-secondary"><button data-action="favorite" data-id="${esc(item.id)}" class="button button-outline">${item.favorite ? "★ В избранном" : "☆ В избранное"}</button><button data-action="add-collection" data-id="${esc(item.id)}" class="button button-outline">＋ В подборку</button></div>
    </div>`;
}

function workshopPage(): string {
  return `<div class="page-heading"><div><div class="eyebrow">STEAM COMMUNITY</div><h1>Steam Workshop</h1><p>Поиск публичных публикаций Wallpaper Engine. Установка выполняется самим клиентом Steam.</p></div><button class="button button-quiet" data-action="open-workshop">Открыть каталог Steam ↗</button></div>
    <div class="workshop-search"><div class="search-box"><span>⌕</span><input id="workshop-search" type="search" placeholder="Найти обои в Workshop..." value="${esc(state.workshopQuery)}"></div><button class="button button-primary" data-action="search-workshop">Искать</button></div>
    <div class="notice"><span>ⓘ</span><div><b>Официальная установка через Steam</b><p>OpenWallpaper не загружает закрытые файлы Workshop. Нажмите «Открыть в Steam», подпишитесь на публикацию, затем обновите библиотеку после загрузки.</p></div></div>
    ${state.workshop.length ? `<div class="workshop-grid">${state.workshop.map((item) => `<article class="workshop-card"><div class="workshop-cover">${item.preview_url && item.preview_url.startsWith("https://") ? `<img src="${esc(item.preview_url)}" alt="" loading="lazy">` : `<div class="cover-placeholder kind-video"><span>STEAM WORKSHOP</span></div>`}</div><div class="workshop-card-body"><b>${esc(item.title)}</b><small>ID ${esc(item.published_file_id)} · ${item.installed ? "Уже загружено" : "Не установлено"}</small><button class="button ${item.installed ? "button-outline" : "button-primary"}" data-action="open-item" data-id="${esc(item.published_file_id)}">${item.installed ? "Открыть в Steam" : "Подписаться в Steam"} ↗</button></div></article>`).join("")}</div>` : `<div class="empty-state workshop-empty"><div class="empty-icon">◈</div><h3>Каталог Workshop</h3><p>Введите запрос для поиска публикаций Wallpaper Engine. Доступность выдачи зависит от Steam Community.</p><button class="button button-outline" data-action="search-workshop">Показать популярное</button></div>`}`;
}

function collectionsPage(): string {
  const selectedCollection = state.collections[0];
  return `<div class="page-heading"><div><div class="eyebrow">ОРГАНИЗАЦИЯ</div><h1>Подборки</h1><p>Группируйте любимые обои по темам, стилям или мониторам.</p></div><button class="button button-primary" data-action="new-collection">＋ Создать подборку</button></div>
    ${state.collections.length ? `<div class="collection-grid">${state.collections.map((c) => `<article class="collection-card"><div class="collection-art"><span>▤</span></div><div><b>${esc(c.name)}</b><small>${c.wallpaper_ids.length} элементов</small></div><button class="button button-outline" data-action="view-collection" data-id="${esc(c.id)}">Открыть</button></article>`).join("")}</div>` : `<div class="empty-state"><div class="empty-icon">▤</div><h3>Подборок пока нет</h3><p>Создайте первую подборку и добавляйте в неё обои из библиотеки.</p><button class="button button-primary" data-action="new-collection">Создать подборку</button></div>`}
    ${selectedCollection ? `<div class="collection-hint">Подборки сохраняются локально в профиле OpenWallpaper.</div>` : ""}`;
}

function settingsPage(): string {
  const s = state.settings!;
  return `<div class="page-heading"><div><div class="eyebrow">ПОВЕДЕНИЕ ПРИЛОЖЕНИЯ</div><h1>Настройки</h1><p>Настройте производительность, запуск и поведение обоев.</p></div><button class="button button-primary" data-action="save-settings">Сохранить изменения</button></div>
    <div class="settings-layout">
      <section class="settings-card"><div class="settings-card-head"><div><b>Запуск и система</b><small>Фоновая работа OpenWallpaper</small></div><span>⌁</span></div>
        ${toggleRow("autostart", "Запускать вместе с Windows", "Приложение стартует в фоне и восстанавливает обои.", s.autostart)}
        ${toggleRow("start_minimized", "Запускать свёрнутым", "Не показывать окно при входе в систему.", s.start_minimized)}
        ${toggleRow("pause_in_games", "Приостанавливать в играх", "Останавливать воспроизведение при активном игровом процессе.", s.pause_in_games)}
        ${toggleRow("pause_on_lock", "Пауза при блокировке Windows", "Приостанавливать обои на экране блокировки.", s.pause_on_lock)}
        ${toggleRow("pause_on_battery", "Пауза от батареи", "Резервная настройка для ноутбуков.", s.pause_on_battery)}
      </section>
      <section class="settings-card"><div class="settings-card-head"><div><b>Производительность</b><small>Снижение нагрузки на систему</small></div><span>◉</span></div>
        <div class="range-row"><div><b>Лимит FPS</b><small>Ограничение кадров для медиадвижка</small></div><strong id="fps-value">${s.fps_limit} FPS</strong><input id="fps_limit" type="range" min="10" max="60" step="5" value="${s.fps_limit}"></div>
        <div class="range-row"><div><b>Длительность слайда</b><small>Время показа одного изображения</small></div><strong id="slide-value">${s.slideshow_seconds} сек.</strong><input id="slideshow_seconds" type="range" min="2" max="30" step="1" value="${s.slideshow_seconds}"></div>
        <div class="range-row"><div><b>Громкость</b><small>Уровень звука видеообоев</small></div><strong id="volume-value">${s.muted ? "Без звука" : `${s.volume}%`}</strong><input id="volume" type="range" min="0" max="100" step="5" value="${s.volume}"></div>
        ${toggleRow("muted", "Выключить звук", "Звук отключён по умолчанию.", s.muted)}
      </section>
      <section class="settings-card full"><div class="settings-card-head"><div><b>Мониторы</b><small>Индивидуальные ограничения FPS и расположение</small></div><span>▣</span></div>
        ${state.monitors.map((m) => `<div class="monitor-row"><div class="monitor-icon">▰</div><div class="monitor-info"><b>${esc(m.name)} ${m.primary ? "· Основной" : ""}</b><small>${m.width} × ${m.height} · ${m.x}, ${m.y}</small></div><label class="monitor-fps">FPS <select data-monitor-fps="${esc(m.id)}"><option value="0">Общий (${s.fps_limit})</option>${[15,20,24,30,40,45,50,60].map((v) => `<option value="${v}" ${(s.monitor_fps[m.id] ?? 0) === v ? "selected" : ""}>${v}</option>`).join("")}</select></label></div>`).join("")}
      </section>
      <section class="settings-card full"><div class="settings-card-head"><div><b>Игровые процессы</b><small>Имена exe-файлов, при которых обои будут останавливаться</small></div><span>⌘</span></div><textarea id="game_processes" class="process-list" spellcheck="false">${esc(s.game_processes.join("\n"))}</textarea></section>
    </div>`;
}
function toggleRow(id: string, title: string, description: string, checked: boolean): string {
  return `<div class="toggle-row"><div><b>${title}</b><small>${description}</small></div><label class="switch"><input type="checkbox" id="${id}" ${checked ? "checked" : ""}><span></span></label></div>`;
}

function render(): void {
  const content = state.page === "library" ? libraryPage() : state.page === "workshop" ? workshopPage() : state.page === "collections" ? collectionsPage() : settingsPage();
  app.innerHTML = shell(content);
  renderToast();
  bindPageControls();
}

function bindPageControls(): void {
  document.querySelectorAll<HTMLElement>("[data-page]").forEach((node) => node.addEventListener("click", () => {
    state.page = node.dataset.page as Page;
    render();
  }));
  document.querySelectorAll<HTMLElement>("[data-action]").forEach((node) => node.addEventListener("click", () => { void handleAction(node); }));
  document.querySelectorAll<HTMLElement>("[data-select]").forEach((node) => {
    node.addEventListener("click", (event) => {
      if ((event.target as HTMLElement).closest("button")) return;
      state.selectedId = node.dataset.select ?? null;
      render();
    });
    node.addEventListener("mouseenter", () => { const video = node.querySelector<HTMLVideoElement>("[data-preview-video]"); if (video) void video.play().catch(() => undefined); });
    node.addEventListener("mouseleave", () => { const video = node.querySelector<HTMLVideoElement>("[data-preview-video]"); if (video) { video.pause(); video.currentTime = 0; } });
  });
  const search = document.querySelector<HTMLInputElement>("#library-search");
  search?.addEventListener("input", () => { const pos = search.selectionStart; state.search = search.value; const scroll = window.scrollY; render(); const next = document.querySelector<HTMLInputElement>("#library-search"); next?.focus(); next?.setSelectionRange(pos, pos); window.scrollTo(0, scroll); });
  document.querySelector<HTMLSelectElement>("#kind-filter")?.addEventListener("change", (e) => { state.filter = (e.target as HTMLSelectElement).value; render(); });
  document.querySelector<HTMLSelectElement>("#monitor-select")?.addEventListener("change", (e) => { state.selectedMonitor = (e.target as HTMLSelectElement).value; });
  document.querySelector<HTMLInputElement>("#workshop-search")?.addEventListener("keydown", (e) => { if (e.key === "Enter") void searchWorkshop(); });
  document.querySelector<HTMLInputElement>("#fps_limit")?.addEventListener("input", (e) => { document.querySelector("#fps-value")!.textContent = `${(e.target as HTMLInputElement).value} FPS`; });
  document.querySelector<HTMLInputElement>("#slideshow_seconds")?.addEventListener("input", (e) => { document.querySelector("#slide-value")!.textContent = `${(e.target as HTMLInputElement).value} сек.`; });
  document.querySelector<HTMLInputElement>("#volume")?.addEventListener("input", (e) => { document.querySelector("#volume-value")!.textContent = `${(e.target as HTMLInputElement).value}%`; });
}

async function handleAction(node: HTMLElement): Promise<void> {
  const action = node.dataset.action;
  const id = node.dataset.id;
  try {
    switch (action) {
      case "import": {
        const selected = await open({ directory: true, multiple: true, title: "Выберите папки с обоями" });
        if (!selected) return;
        const paths = Array.isArray(selected) ? selected : [selected];
        const count = await invoke<number>("add_library_folders", { paths });
        await refresh(); notify(`Библиотека обновлена: ${count} объектов`); break;
      }
      case "rescan": {
        const count = await invoke<number>("rescan_library"); await refresh(); notify(`Найдено объектов: ${count}`); break;
      }
      case "add-web": {
        const url = window.prompt("URL веб-обоев (http или https):", "https://");
        if (!url) return;
        const title = window.prompt("Название веб-обоев:", new URL(url).hostname);
        if (!title) return;
        await invoke("add_web_wallpaper", { url, title }); await refresh(); notify("Веб-обои добавлены"); break;
      }
      case "favorite": {
        if (!id) return; await invoke("toggle_favorite", { wallpaperId: id }); await refresh(); break;
      }
      case "apply": {
        if (!id) return;
        const message = await invoke<string>("apply_wallpaper", { wallpaperId: id, monitorId: state.selectedMonitor });
        notify(message); await refresh(); break;
      }
      case "toggle-pause": {
        state.paused = !state.paused; await invoke("pause_wallpapers", { paused: state.paused }); render(); break;
      }
      case "minimize": await getCurrentWindow().minimize(); break;
      case "hide": await getCurrentWindow().hide(); break;
      case "favorites": state.favoritesOnly = !state.favoritesOnly; render(); break;
      case "clear-collection": state.activeCollection = null; render(); break;
      case "save-settings": await saveSettings(); break;
      case "new-collection": {
        const name = window.prompt("Название подборки:"); if (!name?.trim()) return;
        await invoke("create_collection", { name }); await refresh(); notify("Подборка создана"); break;
      }
      case "add-collection": {
        if (!id) return;
        if (!state.collections.length) { notify("Сначала создайте подборку"); return; }
        const labels = state.collections.map((c) => c.name).join("\n");
        const chosen = window.prompt(`Введите название подборки:\n${labels}`, state.collections[0].name);
        if (!chosen) return;
        const collection = state.collections.find((c) => c.name.toLowerCase() === chosen.toLowerCase());
        if (!collection) { notify("Подборка не найдена"); return; }
        await invoke("add_to_collection", { collectionId: collection.id, wallpaperId: id }); await refresh(); notify("Добавлено в подборку"); break;
      }
      case "view-collection": {
        const collection = state.collections.find((c) => c.id === id); if (!collection) return;
        state.page = "library"; state.favoritesOnly = false; state.search = ""; state.activeCollection = collection.id; render(); notify(`${collection.name}: ${collection.wallpaper_ids.length} обоев`); break;
      }
      case "search-workshop": await searchWorkshop(); break;
      case "open-item": if (id) await invoke("open_workshop_item", { publishedFileId: id }); break;
      case "open-workshop": await invoke("open_external_url", { url: "https://steamcommunity.com/workshop/browse/?appid=431960" }); break;
    }
  } catch (error) { notify(String(error)); }
}

async function searchWorkshop(): Promise<void> {
  const input = document.querySelector<HTMLInputElement>("#workshop-search");
  state.workshopQuery = input?.value ?? state.workshopQuery;
  try {
    const results = await invoke<WorkshopItem[]>("search_workshop", { query: state.workshopQuery, page: 1 });
    state.workshop = results; render();
    if (!results.length) notify("Steam не вернул публикации для этого запроса");
  } catch (error) { notify(`Ошибка Steam Workshop: ${String(error)}`); }
}

async function saveSettings(): Promise<void> {
  const current = state.settings!;
  const value: Settings = {
    ...current,
    autostart: checked("autostart"), start_minimized: checked("start_minimized"), pause_in_games: checked("pause_in_games"),
    pause_on_lock: checked("pause_on_lock"), pause_on_battery: checked("pause_on_battery"), muted: checked("muted"),
    fps_limit: numberValue("fps_limit", current.fps_limit), volume: numberValue("volume", current.volume),
    slideshow_seconds: numberValue("slideshow_seconds", current.slideshow_seconds),
    game_processes: (document.querySelector<HTMLTextAreaElement>("#game_processes")?.value ?? "").split(/\r?\n/).map((s) => s.trim().toLowerCase()).filter(Boolean),
    monitor_fps: { ...current.monitor_fps },
  };
  document.querySelectorAll<HTMLSelectElement>("[data-monitor-fps]").forEach((select) => {
    const monitorId = select.dataset.monitorFps!;
    const fps = Number(select.value);
    if (fps) value.monitor_fps[monitorId] = fps; else delete value.monitor_fps[monitorId];
  });
  await invoke("update_settings", { settings: value });
  state.settings = value; notify("Настройки сохранены");
}
function checked(id: string): boolean { return document.querySelector<HTMLInputElement>(`#${id}`)?.checked ?? false; }
function numberValue(id: string, fallback: number): number { return Number(document.querySelector<HTMLInputElement>(`#${id}`)?.value ?? fallback); }

async function boot(): Promise<void> {
  try {
    await refresh();
    await listen<boolean>("ow:pause", (event) => { state.paused = event.payload; render(); });
    await listen<{ paused: boolean }>("ow:pause-state", (event) => { state.paused = event.payload.paused; render(); });
    await listen<{ message: string }>("ow:restore-error", (event) => {
      notify(`Не удалось восстановить обои: ${event.payload.message}`);
    });
  } catch (error) {
    app.innerHTML = `<div class="startup-error"><h1>OpenWallpaper не смог запуститься</h1><p>${esc(error)}</p><p>Проверьте права приложения и перезапустите его.</p></div>`;
  }
}

void boot();

