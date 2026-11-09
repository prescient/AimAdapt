// Settings, in four tabs: Aim and display (the trainer's settings profiles, one per
// game typically), Goals and time, KovaaK's, and Your data (the last three drawn by
// app.js). Loaded after trainer.js; profiles come with the `trainer` payload and are
// saved with `save_profiles`.

ui.settings = {
  draft: null, // a copy of trainer.profiles being edited
  // Indexes in draft.profiles of the profile shown in the editor, which every edit
  // goes to, and of the active one. Not names: two profiles can share one while you
  // type. draft.active, which is saved, keeps the active one's name.
  selected: 0,
  active: 0,
  problems: [],
  sizeHelper: { monitor: 32, target: 24 },
};

const RESOLUTIONS = ["3840x2160", "2560x1440", "1920x1080", "1728x1080", "1680x1050", "1600x900", "1440x1080", "1280x960", "1280x1024", "1024x768"];

const parseSize = (s) => {
  const m = String(s || "").trim().match(/^(\d{3,5})\s*[x×*]\s*(\d{3,5})$/i);
  return m ? [Number(m[1]), Number(m[2])] : null;
};
const sizeText = (v) => (v ? `${v[0]}x${v[1]}` : "");
const screenPx = () => [Math.round(window.screen.width * devicePixelRatio), Math.round(window.screen.height * devicePixelRatio)];
const gameOf = (id) => ui.trainer.data?.games.find((g) => g.id === id);
const modeLabel = { borderless: "Borderless", exclusive: "Exclusive full screen", windowed: "Window" };

// cm/360 for a profile, matching Profile::cm_per_360 in Rust, or null while a field
// it needs is empty (an emptied field stores null).
function profileCm(p) {
  const g = gameOf(p.game);
  const cm = g && g.yaw > 0 && p.game_sens > 0 ? ((360 / (g.yaw * p.game_sens)) / Math.max(1, p.settings.dpi)) * 2.54 : p.settings.cm_per_360;
  return Number.isFinite(cm) && cm > 0 ? cm : null;
}

const cmText = (cm, digits = 1) => (cm === null ? "–" : cm.toFixed(digits));

// A game's sensitivity for a cm/360 at a DPI.
const gameSens = (g, cm, dpi) => Math.round(((360 / ((cm / 2.54) * Math.max(1, dpi))) / g.yaw) * 1000) / 1000;

function sensText(p) {
  const g = gameOf(p.game);
  const cm = cmText(profileCm(p));
  return g && g.yaw > 0 ? `${g.name} ${num(p.game_sens)} · ${num(p.settings.dpi)} DPI · ${cm} cm/360` : `${cm} cm/360 · ${num(p.settings.dpi)} DPI`;
}

// Called by loadTrainer: start a fresh draft unless one is being edited.
function settingsLoaded() {
  if (!ui.settings.draft || !settingsDirty()) useDraft(ui.trainer.data.profiles);
}

// Edits a copy of these saved profiles, still showing the profile that was shown.
// Saved names are unique (saving refuses two the same), so its name finds it.
function useDraft(profiles) {
  const s = ui.settings;
  const shown = s.draft ? selectedProfile()?.name : null;
  s.draft = structuredClone(profiles);
  s.active = Math.max(0, findProfile(s.draft.active));
  const i = findProfile(shown);
  s.selected = i >= 0 ? i : s.active;
}

function settingsDirty() {
  const d = ui.trainer.data;
  return !d.settings_saved || JSON.stringify(ui.settings.draft) !== JSON.stringify(d.profiles);
}

// Index of the first draft profile with this name, or -1.
const findProfile = (name) => ui.settings.draft.profiles.findIndex((p) => p.name === name);

// The profile shown in the editor (the first if the index has gone).
function shownIndex() {
  const s = ui.settings;
  return s.draft.profiles[s.selected] ? s.selected : 0;
}

function selectedProfile() {
  return ui.settings.draft.profiles[shownIndex()];
}

// ---------- Settings screen ----------

function settingsPage(arg) {
  const tab = SETTINGS_TABS.some(([id]) => id === arg) ? arg : "aim";
  const tr = ui.trainer;
  const ready = Boolean(tr.data && ui.settings.draft);
  // Your data shows the trainer's folders, so every tab asks for its data.
  const wait = waitFor([{ ...trainerNeed(), have: ready }], "Loading settings…");
  const body = tab === "aim"
    ? tr.error ? `<div class="card error">${esc(tr.error)}</div>` : wait ?? settingsAim()
    : { goals: goalsTab, kovaaks: kovaaksTab, data: dataTab }[tab]();
  const actions = tab === "aim" && ready ? `<span class="chips" id="settings-actions">${settingsActions()}</span>` : "";
  return `<div class="page">${head("How AimAdapt is set up", "Settings", actions)}${tabs("settings", SETTINGS_TABS, tab)}${body}</div>`;
}

// The saved settings file couldn't be used (the note says why and what saving does),
// or there isn't one yet.
function settingsNotice(d) {
  if (d.profiles_note) return `<section class="banner warn"><div class="banner-body">${esc(d.profiles_note)}</div></section>`;
  if (d.settings_saved) return "";
  return `<section class="banner info"><div class="banner-body">${d.kovaaks_profile ? "This profile was matched to your latest KovaaK's run. Save to keep it." : "These are defaults. Set your sensitivity and save."}</div></section>`;
}

function settingsAim() {
  const s = ui.settings;
  const d = ui.trainer.data;
  const p = selectedProfile();
  return `
    ${settingsNotice(d)}
    <div id="settings-problems">${settingsProblems()}</div>
    <div class="row">
      <section class="card" style="flex:1 1 280px;max-width:360px;align-self:flex-start">
        <div class="card-head"><h2>Profiles</h2><span class="small muted">One per game</span></div>
        <div class="scenario-list" id="profile-list">${profileList()}</div>
        <div class="field-label small muted">New profile for</div>
        <div class="chips">
          <select class="select" id="new-profile-game" aria-label="Game for the new profile">${d.games.map((g) => `<option value="${g.id}">${esc(g.name)}</option>`).join("")}</select>
          <button class="btn" type="button" data-action="add-profile">Add</button>
        </div>
        <div class="chips">
          <button class="btn" type="button" data-action="duplicate-profile">Duplicate</button>
          <button class="btn" type="button" data-action="delete-profile" ${s.draft.profiles.length < 2 ? "disabled" : ""}>Delete</button>
          ${shownIndex() !== s.active ? `<button class="btn primary" type="button" data-action="activate-profile">Use this one</button>` : ""}
        </div>
      </section>
      <div style="flex:999 1 560px;min-width:0;display:flex;flex-direction:column;gap:16px">${profileEditor(p)}</div>
    </div>`;
}

function settingsActions() {
  const dirty = settingsDirty();
  return `${dirty && ui.trainer.data.settings_saved ? `<button class="btn" type="button" data-action="revert-settings">Undo changes</button>` : ""}
    <button class="btn ${dirty ? "primary" : ""}" type="button" data-action="save-settings" ${dirty ? "" : "disabled"}>${dirty ? "Save settings" : "Saved"}</button>`;
}

function settingsProblems() {
  const ps = ui.settings.problems;
  return ps.length ? `<section class="banner"><div><div class="banner-title">Fix before saving</div><ul class="problems">${ps.map((x) => `<li>${esc(x)}</li>`).join("")}</ul></div></section>` : "";
}

// Each profile, picked by its place in the list.
function profileList() {
  const s = ui.settings;
  const shown = shownIndex();
  return s.draft.profiles.map((x, i) => `<button type="button" class="scenario-item ${i === shown ? "on" : ""}" data-action="select-profile" data-index="${i}">
    <span class="main"><span class="name">${esc(x.name)} ${i === s.active ? '<span class="tag">active</span>' : ""}</span>
    <span class="sub">${esc(sensText(x))}</span></span></button>`).join("");
}

// Refreshes the parts that change while typing, without redrawing the form.
function refreshSettingsBits() {
  if (route().view !== "settings" || !document.getElementById("settings-actions")) return;
  const set = (id, html) => {
    const el = document.getElementById(id);
    if (el) el.innerHTML = html;
  };
  set("settings-actions", settingsActions());
  set("settings-problems", settingsProblems());
  set("profile-list", profileList());
  const out = document.getElementById("cm-readout");
  if (out) out.textContent = cmText(profileCm(selectedProfile()), 2);
  const h2 = document.getElementById("profile-title");
  if (h2) h2.textContent = selectedProfile().name;
}

function profileEditor(p) {
  const d = ui.trainer.data;
  const g = gameOf(p.game);
  const st = p.settings;
  const disp = st.display;
  const screen = screenPx();
  const h = ui.settings.sizeHelper;
  const k = d.kovaaks_profile;
  const num_ = (label, key, attrs, unit = "", help = "") =>
    `<label class="form-row"><span>${label}</span><span class="field"><input type="number" data-pfield="${key}" value="${esc(key.split(".").reduce((o, x) => o?.[x], p) ?? "")}" ${attrs}>${unit ? `<span class="muted small">${unit}</span>` : ""}</span>${help ? `<span class="help">${help}</span>` : ""}</label>`;
  return `
    <section class="card">
      <div class="card-head" style="align-items:center"><h2 id="profile-title">${esc(p.name)}</h2>
        ${k ? `<button class="btn" type="button" data-action="match-kovaaks" title="${esc(sensText(k))}">Match my KovaaK's settings</button>` : ""}</div>
      <div class="form-grid">
        <label class="form-row"><span>Profile name</span><span class="field"><input type="text" data-pfield="name" value="${esc(p.name)}"></span></label>
        <label class="form-row"><span>Game</span><select class="select" data-pfield="game">${d.games.map((x) => `<option value="${x.id}" ${x.id === p.game ? "selected" : ""}>${esc(x.name)}</option>`).join("")}</select>
          <span class="help">Type your sensitivity exactly as it is in that game.</span></label>
      </div>
    </section>
    <section class="card"><h2>Aim</h2>
      <div class="form-grid">
        ${g && g.yaw > 0
          ? num_(`${esc(g.name)} sensitivity`, "game_sens", 'min="0.001" max="1000" step="0.01"', "", `= <span id="cm-readout" class="num">${cmText(profileCm(p), 2)}</span> cm/360 at your DPI`)
          : num_("Sensitivity", "settings.cm_per_360", 'min="1" max="500" step="0.1"', "cm/360", "Centimeters of mouse movement for a full turn.")}
        ${num_("Mouse DPI", "settings.dpi", 'min="50" max="50000" step="50"')}
        <label class="form-row"><span>Adaptive target mix</span><select class="select" data-pfield="settings.challenge">
          ${[["steady", "Steady: mostly near your rating"], ["balanced", "Balanced: some stretch and easy runs"], ["push", "Push: more stretch runs"]].map(([v, l]) => `<option value="${v}" ${st.challenge === v ? "selected" : ""}>${l}</option>`).join("")}
        </select><span class="help">Stretch targets are faster than you can reliably hit. Hitting them moves your rating the most.</span></label>
        ${num_("Field of view", "settings.fov", 'min="30" max="150" step="0.1"', "° horizontal (16:9)", `${esc(g?.fov_note ?? "")}${g?.fov ? ` <button class="link-btn" type="button" data-action="game-fov">Use ${g.fov}</button>` : ""}`)}
      </div>
    </section>
    <section class="card"><h2>Crosshair</h2>
      <div class="form-grid">
        <label class="form-row"><span>Color</span><span class="field"><input type="color" data-pfield="settings.crosshair.color" value="${esc(st.crosshair.color)}"></span></label>
        ${num_("Arm length", "settings.crosshair.size", 'min="0" max="40" step="1"', "px", "0 for a dot only.")}
        ${num_("Thickness", "settings.crosshair.thickness", 'min="1" max="10" step="1"', "px")}
        ${num_("Gap", "settings.crosshair.gap", 'min="0" max="30" step="1"', "px")}
        <label class="form-row check"><input type="checkbox" data-pfield="settings.crosshair.dot" ${st.crosshair.dot ? "checked" : ""}><span>Center dot</span></label>
        <label class="form-row check"><input type="checkbox" data-pfield="settings.show_fps" ${st.show_fps ? "checked" : ""}><span>Show FPS (F2 in game)</span></label>
      </div>
      ${crosshairPreview(st.crosshair)}
    </section>
    <section class="card"><h2>Display</h2>
      <div class="form-grid">
        <label class="form-row"><span>Mode</span><select class="select" data-pfield="settings.display.mode">
          ${Object.entries(modeLabel).map(([v, l]) => `<option value="${v}" ${disp.mode === v ? "selected" : ""}>${l}</option>`).join("")}
        </select><span class="help">Exclusive lets you use a different resolution, like stretched 4:3.</span></label>
        <label class="form-row"><span>Resolution</span><span class="field"><input type="text" list="resolutions" data-pfield="settings.display.resolution" data-kind="size" placeholder="${disp.mode === "windowed" ? "1600x900" : `Desktop (${screen[0]}x${screen[1]})`}" value="${esc(sizeText(disp.resolution))}"></span>
          <span class="help">${disp.mode === "borderless" ? "Borderless always uses the desktop resolution." : "Type any size, like 1440x1080."}</span></label>
        ${disp.mode === "exclusive" ? num_("Refresh rate", "settings.display.refresh_hz", 'min="24" max="1000" step="1" placeholder="Highest"', "Hz") : ""}
        <label class="form-row"><span>Play area</span><span class="field"><input type="text" list="resolutions" data-pfield="settings.display.play_area" data-kind="size" placeholder="Whole screen" value="${esc(sizeText(disp.play_area))}"></span>
          <span class="help">Draws the game in a smaller area with black around it, so a big monitor plays like a smaller one.</span></label>
        ${disp.play_area ? playAreaPosition(disp, screen) : ""}
      </div>
      <div class="size-helper">
        <span>Fit the play area to a</span>
        <span class="field small-field"><input type="number" min="10" max="100" step="0.5" data-helper="target" value="${h.target}"><span class="muted small">in</span></span>
        <span>screen on my</span>
        <span class="field small-field"><input type="number" min="10" max="120" step="0.5" data-helper="monitor" value="${h.monitor}"><span class="muted small">in</span></span>
        <span>monitor (${screen[0]}x${screen[1]})</span>
        <button class="btn" type="button" data-action="fit-play-area">Set play area</button>
        ${disp.play_area ? `<button class="btn" type="button" data-action="clear-play-area">Use whole screen</button>` : ""}
      </div>
      <datalist id="resolutions">${RESOLUTIONS.map((r) => `<option value="${r}">`).join("")}</datalist>
      <div class="small muted">Sensitivity is in degrees per mouse count, so it stays the same at any resolution or play-area size. FOV is horizontal on a 16:9 screen; 4:3 areas see a little less at the sides, as in most shooters.</div>
    </section>`;
}

// Where the play area sits on the screen: a share of the black space to its left
// and above it, 0–1 each (unset = centered). Matches Display::play_area_in in Rust.
const AREA_SPOTS = { center: [0.5, 0.5], top: [0.5, 0], bottom: [0.5, 1] };
function areaSpot(pos) {
  const [x, y] = pos ?? [0.5, 0.5];
  return Object.entries(AREA_SPOTS).find(([, v]) => v[0] === x && v[1] === y)?.[0] ?? "custom";
}

function playAreaPosition(disp, screen) {
  const pos = disp.play_area_position ?? [0.5, 0.5];
  const spot = ui.settings.areaCustom ? "custom" : areaSpot(disp.play_area_position);
  const pct = (f) => Math.round(f * 1000) / 10;
  return `<label class="form-row"><span>Play area position</span><select class="select" data-action-change="area-spot">
      ${[["center", "Centered"], ["top", "Top of the screen"], ["bottom", "Bottom of the screen"], ["custom", "Custom"]].map(([v, l]) => `<option value="${v}" ${spot === v ? "selected" : ""}>${l}</option>`).join("")}
    </select><span class="help">Where the play area sits on your screen, for example near the top if that's where you keep your game window.</span></label>
    ${spot === "custom" ? `<div class="form-row"><span>Custom position</span><span class="area-custom">
      <span class="field small-field"><input type="number" min="0" max="100" step="1" data-area="0" value="${pct(pos[0])}" aria-label="From the left"><span class="muted small">% from left</span></span>
      <span class="field small-field"><input type="number" min="0" max="100" step="1" data-area="1" value="${pct(pos[1])}" aria-label="From the top"><span class="muted small">% from top</span></span></span>
      <span class="help">0% puts it against the left or top edge, 50% centers it, 100% puts it against the right or bottom.</span></div>` : ""}
    ${areaPreview(disp, screen)}`;
}

// A small picture of the screen with the play area drawn where it will be.
function areaPreview(disp, screen) {
  const pos = disp.play_area_position ?? [0.5, 0.5];
  const W = 160, H = Math.round((W * screen[1]) / screen[0]);
  const [aw, ah] = [Math.min(1, disp.play_area[0] / screen[0]) * W, Math.min(1, disp.play_area[1] / screen[1]) * H];
  const [ax, ay] = [(W - aw) * pos[0], (H - ah) * pos[1]];
  return `<div class="area-preview" aria-label="Play area preview"><svg viewBox="0 0 ${W} ${H}" width="${W}" height="${H}"><rect width="${W}" height="${H}" rx="3" fill="#000" stroke="var(--control)"/><rect x="${ax.toFixed(1)}" y="${ay.toFixed(1)}" width="${aw.toFixed(1)}" height="${ah.toFixed(1)}" fill="#3a3f4b" stroke="var(--rust)"/></svg>
      <span class="small muted">Your screen (${screen[0]}x${screen[1]}) with the play area</span></div>`;
}

function crosshairPreview(c) {
  const S = 120, m = S / 2, th = Math.max(1, c.thickness), off = c.gap + c.size / 2 + th / 2;
  const bar = (x, y, w, h) => `<rect x="${m + x - w / 2}" y="${m + y - h / 2}" width="${w}" height="${h}" fill="${esc(c.color)}" stroke="rgba(0,0,0,.6)" stroke-width="1"/>`;
  const arms = c.size > 0 ? bar(-off, 0, c.size, th) + bar(off, 0, c.size, th) + bar(0, -off, th, c.size) + bar(0, off, th, c.size) : "";
  return `<div class="crosshair-preview" aria-label="Crosshair preview"><svg viewBox="0 0 ${S} ${S}" width="${S}" height="${S}"><rect width="${S}" height="${S}" fill="#3a3f4b"/>${arms}${c.dot ? bar(0, 0, th, th) : ""}</svg><span class="small muted">Preview at actual size</span></div>`;
}

let profileTimer = null;
function recheckProfiles() {
  clearTimeout(profileTimer);
  profileTimer = setTimeout(async () => {
    try {
      const [problems] = await invoke("check_profiles", { profiles: ui.settings.draft });
      ui.settings.problems = problems;
    } catch (e) {
      ui.settings.problems = [String(e)];
    }
    refreshSettingsBits();
  }, 300);
}

function setProfileField(el) {
  const p = selectedProfile();
  const key = el.dataset.pfield;
  let v;
  if (el.type === "checkbox") v = el.checked;
  else if (el.dataset.kind === "size") v = parseSize(el.value);
  else if (el.type === "number") v = el.value === "" ? null : Number(el.value);
  else v = el.value;
  if (key === "name") {
    // It stays shown (by its place), and keeps the active mark if it has it.
    p.name = v;
    if (ui.settings.active === shownIndex()) ui.settings.draft.active = v;
    return;
  }
  if (key === "game") {
    // Keep the same feel when switching games: convert the current cm/360.
    const cm = profileCm(p);
    p.game = v;
    const g = gameOf(v);
    if (cm === null) return;
    if (g && g.yaw > 0) p.game_sens = gameSens(g, cm, p.settings.dpi);
    else p.settings.cm_per_360 = Math.round(cm * 100) / 100;
    return;
  }
  const keys = key.split(".");
  const last = keys.pop();
  keys.reduce((o, x) => o[x], p)[last] = v;
}

function uniqueName(base) {
  const names = ui.settings.draft.profiles.map((p) => p.name.toLowerCase());
  let name = base, n = 2;
  while (names.includes(name.toLowerCase())) name = `${base} ${n++}`;
  return name;
}

document.addEventListener("click", (e) => {
  const btn = e.target.closest("[data-action]");
  if (!btn || !ui.settings.draft) return;
  const s = ui.settings;
  const action = btn.dataset.action;
  const p = () => selectedProfile();
  if (action === "select-profile") s.selected = Number(btn.dataset.index);
  else if (action === "activate-profile") {
    s.active = shownIndex();
    s.draft.active = p().name;
  } else if (action === "add-profile") {
    const g = gameOf(document.getElementById("new-profile-game").value);
    const base = structuredClone(p());
    const prof = { name: uniqueName(g.name), game: g.id, game_sens: 1, settings: base.settings };
    if (g.fov) prof.settings.fov = g.fov;
    const cm = profileCm(base);
    if (g.yaw > 0 && cm !== null) prof.game_sens = gameSens(g, cm, base.settings.dpi);
    s.draft.profiles.push(prof);
    s.selected = s.draft.profiles.length - 1;
  } else if (action === "duplicate-profile") {
    const copy = structuredClone(p());
    copy.name = uniqueName(`${copy.name} copy`);
    s.draft.profiles.push(copy);
    s.selected = s.draft.profiles.length - 1;
  } else if (action === "delete-profile") {
    if (s.draft.profiles.length < 2 || !confirm(`Delete the ${p().name} profile?`)) return;
    // Only the one shown, even if another has its name.
    const i = shownIndex();
    s.draft.profiles.splice(i, 1);
    if (s.active === i) s.active = 0;
    else if (s.active > i) s.active -= 1;
    s.draft.active = s.draft.profiles[s.active].name;
    s.selected = s.active;
  } else if (action === "match-kovaaks") {
    const k = ui.trainer.data.kovaaks_profile;
    Object.assign(p(), { game: k.game, game_sens: k.game_sens });
    Object.assign(p().settings, { cm_per_360: k.settings.cm_per_360, dpi: k.settings.dpi, fov: k.settings.fov });
  } else if (action === "game-fov") p().settings.fov = gameOf(p().game).fov;
  else if (action === "fit-play-area") {
    const px = screenPx();
    const k = Math.min(1, Math.max(0.1, s.sizeHelper.target / s.sizeHelper.monitor));
    p().settings.display.play_area = k >= 0.999 ? null : [Math.round(px[0] * k), Math.round(px[1] * k)];
  } else if (action === "clear-play-area") p().settings.display.play_area = null;
  else if (action === "revert-settings") {
    useDraft(ui.trainer.data.profiles);
    s.problems = [];
  } else if (action === "save-settings") {
    saveProfiles();
    return;
  } else return;
  render();
  recheckProfiles();
});

async function saveProfiles() {
  const s = ui.settings;
  try {
    await invoke("save_profiles", { profiles: s.draft });
    s.problems = [];
    ui.trainer.message = "Settings saved.";
    await loadTrainer();
    if (!ui.trainer.error) useDraft(ui.trainer.data.profiles);
  } catch (e) {
    s.problems = String(e).split("; ");
  }
  render();
}

document.addEventListener("input", (e) => {
  const el = e.target;
  if (el.dataset.pfield) {
    setProfileField(el);
    refreshSettingsBits();
    recheckProfiles();
  } else if (el.dataset.helper) ui.settings.sizeHelper[el.dataset.helper] = Number(el.value);
  else if (el.dataset.area !== undefined && el.value !== "") {
    const d = selectedProfile().settings.display;
    const pos = [...(d.play_area_position ?? [0.5, 0.5])];
    pos[Number(el.dataset.area)] = Math.min(1, Math.max(0, Number(el.value) / 100));
    d.play_area_position = pos;
    const prev = document.querySelector(".area-preview");
    if (prev) prev.outerHTML = areaPreview(d, screenPx());
    refreshSettingsBits();
    recheckProfiles();
  }
});

document.addEventListener("change", (e) => {
  const el = e.target;
  if (el.dataset.actionChange === "area-spot") {
    const d = selectedProfile().settings.display;
    ui.settings.areaCustom = el.value === "custom";
    if (el.value !== "custom") d.play_area_position = el.value === "center" ? null : AREA_SPOTS[el.value];
    render();
    recheckProfiles();
  }
  else if (el.dataset.pfield && (el.tagName === "SELECT" || el.type === "checkbox" || el.type === "color")) {
    setProfileField(el);
    render();
    recheckProfiles();
  }
});
