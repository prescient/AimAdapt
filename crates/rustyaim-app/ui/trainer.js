// Play screen and scenario editor for the built-in trainer. Loaded after app.js and
// uses its helpers (esc, num, acc, head, invoke, ui, render, fmtShort, dt).
//
// The editor's form is built from `trainer.fields`, the engine's own list of every
// scenario setting with its limits and help text, so new engine options show up here
// without UI changes.

ui.trainer = {
  data: null,
  error: null,
  filter: "all",
  edit: null, // { id, builtin, scenario, problems, saving }
  settings: null, // draft of engine settings
  message: null,
};

const CATEGORIES = [
  ["all", "All"],
  ["clicking", "Clicking"],
  ["tracking", "Tracking"],
  ["switching", "Switching"],
  ["mine", "Yours"],
];

async function loadTrainer() {
  try {
    const data = await invoke("trainer");
    // cm/360 converted from game sensitivity has long decimals; two places is plenty.
    for (const s of [data.settings, data.kovaaks_settings]) if (s) s.cm_per_360 = Math.round(s.cm_per_360 * 100) / 100;
    ui.trainer.data = data;
    ui.trainer.error = null;
    if (!ui.trainer.settings) ui.trainer.settings = structuredClone(ui.trainer.data.settings);
  } catch (e) {
    ui.trainer.error = String(e);
  }
  const v = route().view;
  if (v === "play" || v === "edit") render();
}

const getPath = (obj, path) => path.split(".").reduce((o, k) => (o == null ? undefined : o[k]), obj);
function setPath(obj, path, value) {
  const keys = path.split(".");
  const last = keys.pop();
  const target = keys.reduce((o, k) => (o[k] ??= {}), obj);
  target[last] = value;
}

// Angular width of a target in degrees at the middle of its spawn distance.
function targetDegrees(s) {
  const d = (s.targets.spawn.distance[0] + s.targets.spawn.distance[1]) / 2;
  return (2 * Math.atan(s.targets.radius / d) * 180) / Math.PI;
}

function scenarioFacts(s) {
  const t = s.targets;
  const parts = [
    `${num(s.timing.duration_s)} s`,
    `${t.count} target${t.count === 1 ? "" : "s"}`,
    `${targetDegrees(s).toFixed(1)}° wide`,
    t.movement.kind === "static" ? "still" : `${t.movement.kind} ${num(t.movement.speed[0])}–${num(t.movement.speed[1])} m/s`,
    s.weapon.kind === "beam" ? "beam" : s.weapon.trigger === "click" ? "click" : `${s.weapon.trigger} fire`,
  ];
  if (t.lifetime_s > 0) parts.push(`${num(t.lifetime_s)} s lifetime`);
  return parts.join(" · ");
}

// ---------- Play ----------

function play() {
  const tr = ui.trainer;
  if (tr.error) return `<div class="page">${head("Built-in aim trainer", "Play")}<div class="card error">${esc(tr.error)}</div></div>`;
  if (!tr.data) {
    loadTrainer();
    return `<div class="empty-state">Loading scenarios…</div>`;
  }
  const d = tr.data;
  const list = d.scenarios.filter((e) => {
    if (tr.filter === "all") return true;
    if (tr.filter === "mine") return !e.builtin;
    return e.scenario?.category === tr.filter;
  });
  return `<div class="page">
    ${head("Built-in aim trainer", "Play", `<button class="btn primary" type="button" data-action="new-scenario">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"></path></svg>New scenario</button>`)}
    ${d.playing ? `<section class="banner info"><div><div class="banner-title">The trainer is open</div><div class="banner-body">Finish or press Esc to come back. Your run is saved when the timer ends.</div></div></section>` : ""}
    ${tr.message ? `<section class="banner"><div class="banner-body">${esc(tr.message)}</div></section>` : ""}
    ${settingsCard()}
    <div class="chips" role="group" aria-label="Filter scenarios">
      ${CATEGORIES.map(([k, label]) => `<button class="chip ${tr.filter === k ? "on" : ""}" type="button" data-action="trainer-filter" data-filter="${k}" aria-pressed="${tr.filter === k}">${label}</button>`).join("")}
    </div>
    <div class="grid">${list.map(scenarioCard).join("") || `<div class="card note-body">No scenarios here yet. Use New scenario to make one.</div>`}</div>
    <div class="small muted">Your scenarios are TOML files in <span class="mono">${esc(d.scenarios_dir)}</span>. See SCENARIOS.md for every setting.</div>
  </div>`;
}

function scenarioCard(e) {
  const s = e.scenario;
  const id = esc(e.id);
  if (!s) {
    return `<section class="card"><div class="card-head"><h3>${esc(e.id.replace(/^user:/, ""))}</h3><span class="tag grey">Can't load</span></div>
      <div class="note-body error">${esc(e.error)}</div>
      <div class="chips"><button class="btn" type="button" data-action="delete-scenario" data-id="${id}">Delete file</button></div></section>`;
  }
  return `<section class="card scenario-card">
    <div class="card-head"><h3>${esc(s.name)}</h3><span>${e.builtin ? "" : '<span class="tag gold">Yours</span> '}<span class="tag grey">${esc(s.category)}</span></span></div>
    <div class="note-body">${esc(s.description || "No description.")}</div>
    <div class="small muted">${esc(scenarioFacts(s))}</div>
    <div class="small">${e.runs ? `Best <span class="num">${num(e.best)}</span> · ${e.runs} run${e.runs === 1 ? "" : "s"} · last ${esc(fmtShort(dt(e.last_played)))}` : '<span class="muted">Not played yet</span>'}</div>
    <div class="chips">
      <button class="btn primary" type="button" data-action="play" data-id="${id}" ${ui.trainer.data.playing ? "disabled" : ""}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4l13 8-13 8z"></path></svg>Play</button>
      <a class="btn" href="#edit/${encodeURIComponent(e.id)}">${e.builtin ? "Copy &amp; edit" : "Edit"}</a>
      ${e.runs ? `<a class="btn" href="#progress/${encodeURIComponent(s.name)}">History</a>` : ""}
    </div>
  </section>`;
}

function settingsCard() {
  const d = ui.trainer.data;
  const s = ui.trainer.settings;
  const k = d.kovaaks_settings;
  const dirty = JSON.stringify(s) !== JSON.stringify(d.settings) || !d.settings_saved;
  const field = (label, key, attrs, unit = "") =>
    `<label class="form-row"><span>${label}</span><span class="field"><input type="number" data-setting="${key}" value="${esc(getPath(s, key))}" ${attrs}>${unit ? `<span class="muted small">${unit}</span>` : ""}</span></label>`;
  return `<section class="card">
    <div class="card-head" style="align-items:center"><h2>Your settings</h2>
      <span class="chips">
        ${k ? `<button class="btn" type="button" data-action="match-kovaaks" title="${esc(`${k.cm_per_360.toFixed(1)} cm/360, ${k.dpi} DPI, FOV ${k.fov}`)}">Match KovaaK's</button>` : ""}
        <button class="btn ${dirty ? "primary" : ""}" type="button" data-action="save-settings" ${dirty ? "" : "disabled"}>${d.settings_saved && !dirty ? "Saved" : "Save settings"}</button>
      </span></div>
    ${d.settings_saved ? "" : `<div class="small muted">${k ? "Matched to your latest KovaaK's run. Save to keep them." : "Defaults. Set your sensitivity and save."}</div>`}
    <div class="form-grid">
      ${field("Sensitivity", "cm_per_360", 'min="1" max="500" step="0.1"', "cm/360")}
      ${field("Mouse DPI", "dpi", 'min="50" max="50000" step="50"')}
      ${field("Field of view", "fov", 'min="30" max="150" step="1"', "° horizontal (16:9)")}
      <label class="form-row"><span>Crosshair color</span><span class="field"><input type="color" data-setting="crosshair.color" value="${esc(s.crosshair.color)}"></span></label>
      ${field("Crosshair size", "crosshair.size", 'min="0" max="40" step="1"', "px")}
      <label class="form-row check"><input type="checkbox" data-setting="fullscreen" ${s.fullscreen ? "checked" : ""}><span>Full screen</span></label>
      <label class="form-row check"><input type="checkbox" data-setting="show_fps" ${s.show_fps ? "checked" : ""}><span>Show FPS (F2 in game)</span></label>
    </div>
    <div class="small muted">In the trainer: Esc quits, R restarts, Space plays again after a run.</div>
  </section>`;
}

// ---------- Editor ----------

function edit(id) {
  const tr = ui.trainer;
  if (!tr.data) {
    loadTrainer();
    return `<div class="empty-state">Loading…</div>`;
  }
  if (!tr.edit || tr.edit.forId !== id) {
    const entry = tr.data.scenarios.find((e) => e.id === id);
    let scenario;
    if (id === "new") scenario = structuredClone(tr.data.scenarios.find((e) => e.id === "builtin:static-grid")?.scenario);
    else scenario = structuredClone(entry?.scenario);
    if (!scenario) return `<div class="page">${head("Scenario editor", "Not found")}<div class="card note-body">That scenario couldn't be loaded. <a href="#play">Back to Play</a></div></div>`;
    if (id === "new") Object.assign(scenario, { name: "My scenario", author: "", description: "", tags: [] });
    else if (entry.builtin) Object.assign(scenario, { name: `${scenario.name} (my copy)`, author: "" });
    tr.edit = { forId: id, replace: entry && !entry.builtin ? id : null, builtin: !!entry?.builtin, scenario, problems: [] };
  }
  const e = tr.edit;
  const groups = [];
  for (const f of tr.data.fields) {
    let g = groups.find((x) => x.name === f.group);
    if (!g) groups.push((g = { name: f.group, fields: [] }));
    g.fields.push(f);
  }
  return `<div class="page">
    ${head(e.replace ? "Edit scenario" : e.builtin ? "Copy of a built-in scenario" : "New scenario", esc(e.scenario.name || "Untitled"), `<span class="chips">
      <a class="btn" href="#play">Cancel</a>
      ${e.replace ? `<button class="btn" type="button" data-action="delete-scenario" data-id="${esc(e.replace)}">Delete</button>` : ""}
      <button class="btn" type="button" data-action="save-scenario">Save</button>
      <button class="btn primary" type="button" data-action="save-play">Save &amp; play</button></span>`)}
    ${e.builtin ? `<section class="banner info"><div class="banner-body">Built-in scenarios stay as they are. Saving makes your own copy.</div></section>` : ""}
    <div id="edit-status">${editStatus()}</div>
    <div class="row">
      <div class="grow-3 editor-groups">${groups.map((g) => `<section class="card"><h2>${esc(g.name)}</h2><div class="form-grid">${g.fields.map((f) => fieldInput(f, getPath(e.scenario, f.path))).join("")}</div></section>`).join("")}</div>
    </div>
  </div>`;
}

function editStatus() {
  const e = ui.trainer.edit;
  if (!e) return "";
  const s = e.scenario;
  const problems = e.problems.length
    ? `<section class="banner"><div><div class="banner-title">Fix before saving</div><ul class="problems">${e.problems.map((p) => `<li>${esc(p)}</li>`).join("")}</ul></div></section>`
    : "";
  let facts = "";
  try {
    facts = `<div class="small muted">${esc(scenarioFacts(s))}</div>`;
  } catch {
    facts = "";
  }
  return problems + facts;
}

function fieldInput(f, value) {
  const id = `f-${f.path.replace(/\./g, "-")}`;
  const help = f.help ? `<span class="help">${esc(f.help)}</span>` : "";
  const unit = f.unit ? `<span class="muted small">${esc(f.unit)}</span>` : "";
  const numAttrs = (min, max, step) => `min="${min}" max="${max}" step="${step}"`;
  let control;
  switch (f.kind) {
    case "text":
      control = `<span class="field"><input id="${id}" type="text" data-path="${f.path}" data-kind="text" value="${esc(value)}"></span>`;
      break;
    case "long_text":
      control = `<textarea id="${id}" class="textarea" rows="3" data-path="${f.path}" data-kind="text">${esc(value)}</textarea>`;
      break;
    case "tags":
      control = `<span class="field"><input id="${id}" type="text" data-path="${f.path}" data-kind="tags" value="${esc((value || []).join(", "))}" placeholder="micro, flick"></span>`;
      break;
    case "color":
      control = `<span class="field"><input id="${id}" type="color" data-path="${f.path}" data-kind="text" value="${esc(value)}"></span>`;
      break;
    case "number":
      control = `<span class="field"><input id="${id}" type="number" data-path="${f.path}" data-kind="number" value="${esc(value)}" ${numAttrs(f.min, f.max, f.step)}>${unit}</span>`;
      break;
    case "integer":
      control = `<span class="field"><input id="${id}" type="number" data-path="${f.path}" data-kind="integer" value="${esc(value)}" ${numAttrs(f.min, f.max, 1)}></span>`;
      break;
    case "range":
      control = `<span class="pair">${[0, 1].map((i) => `<span class="field"><input type="number" aria-label="${esc(f.label)} ${i ? "max" : "min"}" data-path="${f.path}" data-kind="range" data-index="${i}" value="${esc(value[i])}" ${numAttrs(f.min, f.max, f.step)}><span class="muted small">${i ? "max" : "min"}</span></span>`).join("")}${unit}</span>`;
      break;
    case "size3":
      control = `<span class="pair">${["W", "H", "D"].map((l, i) => `<span class="field"><input type="number" aria-label="${esc(f.label)} ${l}" data-path="${f.path}" data-kind="range" data-index="${i}" value="${esc(value[i])}" ${numAttrs(f.min, f.max, f.step)}><span class="muted small">${l}</span></span>`).join("")}${unit}</span>`;
      break;
    case "choice":
      control = `<select id="${id}" class="select" data-path="${f.path}" data-kind="text">${f.options.map((o) => `<option value="${esc(o.value)}" ${o.value === value ? "selected" : ""} title="${esc(o.help)}">${esc(o.label)}</option>`).join("")}</select>`;
      break;
    default:
      control = `<span class="muted">${esc(JSON.stringify(value))}</span>`;
  }
  return `<div class="form-row"><label for="${id}">${esc(f.label)}</label>${control}${help}</div>`;
}

let checkTimer = null;
function onEditorInput(el) {
  const e = ui.trainer.edit;
  if (!e) return;
  const path = el.dataset.path;
  let v;
  switch (el.dataset.kind) {
    case "number":
    case "integer":
      v = el.value === "" ? 0 : Number(el.value);
      break;
    case "tags":
      v = el.value.split(",").map((t) => t.trim()).filter(Boolean);
      break;
    case "range": {
      v = [...getPath(e.scenario, path)];
      v[Number(el.dataset.index)] = el.value === "" ? 0 : Number(el.value);
      break;
    }
    default:
      v = el.value;
  }
  setPath(e.scenario, path, v);
  clearTimeout(checkTimer);
  checkTimer = setTimeout(async () => {
    try {
      e.problems = tauri ? await invoke("check_scenario", { scenario: e.scenario }) : [];
    } catch (err) {
      e.problems = [String(err)];
    }
    const box = document.getElementById("edit-status");
    if (box) box.innerHTML = editStatus();
    const h1 = document.querySelector(".page-head h1");
    if (h1) h1.textContent = e.scenario.name || "Untitled";
  }, 150);
}

async function saveScenario(thenPlay) {
  const e = ui.trainer.edit;
  try {
    const id = await invoke("save_scenario", { scenario: e.scenario, replace: e.replace });
    ui.trainer.edit = null;
    ui.trainer.message = `Saved ${e.scenario.name}.`;
    await loadTrainer();
    location.hash = "#play";
    if (thenPlay) await startPlay(id);
  } catch (err) {
    e.problems = String(err).split("; ");
    document.getElementById("edit-status").innerHTML = editStatus();
    window.scrollTo(0, 0);
  }
}

async function startPlay(id) {
  try {
    await invoke("play", { id });
    ui.trainer.message = null;
  } catch (e) {
    ui.trainer.message = String(e);
  }
  await loadTrainer();
}

function onSettingInput(el) {
  const s = ui.trainer.settings;
  const key = el.dataset.setting;
  const v = el.type === "checkbox" ? el.checked : el.type === "number" ? Number(el.value) : el.value;
  setPath(s, key, v);
}

document.addEventListener("click", async (e) => {
  const btn = e.target.closest("[data-action]");
  if (!btn) return;
  const action = btn.dataset.action;
  const tr = ui.trainer;
  if (action === "trainer-filter") {
    tr.filter = btn.dataset.filter;
    render();
  } else if (action === "play") startPlay(btn.dataset.id);
  else if (action === "new-scenario") location.hash = "#edit/new";
  else if (action === "save-scenario") saveScenario(false);
  else if (action === "save-play") saveScenario(true);
  else if (action === "delete-scenario") {
    const id = btn.dataset.id;
    if (!confirm(`Delete ${id.replace(/^user:/, "")}? This removes the file; your past runs stay in your history.`)) return;
    try {
      await invoke("delete_scenario", { id });
      tr.edit = null;
      tr.message = "Scenario deleted.";
    } catch (err) {
      tr.message = String(err);
    }
    location.hash = "#play";
    loadTrainer();
  } else if (action === "match-kovaaks") {
    const k = tr.data.kovaaks_settings;
    Object.assign(tr.settings, { cm_per_360: Math.round(k.cm_per_360 * 10) / 10, dpi: k.dpi, fov: k.fov });
    render();
  } else if (action === "save-settings") {
    try {
      await invoke("save_engine_settings", { settings: tr.settings });
      tr.message = "Settings saved.";
      tr.settings = null;
    } catch (err) {
      tr.message = String(err);
    }
    loadTrainer();
  }
});

document.addEventListener("input", (e) => {
  if (e.target.dataset.path) onEditorInput(e.target);
  else if (e.target.dataset.setting) onSettingInput(e.target);
});

document.addEventListener("change", (e) => {
  if (e.target.dataset.setting) {
    onSettingInput(e.target);
    if (route().view === "play") render();
  }
});

if (tauri) {
  tauri.event.listen("trainer-closed", async () => {
    await load();
    await loadTrainer();
  });
}
