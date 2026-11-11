// The built-in trainer's data, the scenario and workout editors, and the actions
// that start the trainer. Loaded after app.js and uses its helpers (esc, num, head,
// invoke, ui, render, route, fmtShort, dt). The Scenarios page (scenarios.js) lists
// and shows what this loads.
//
// The scenario editor's form is built from `trainer.fields`, the engine's own list of
// every scenario setting with its limits and help text, so new engine options show up
// here without UI changes.

ui.trainer = {
  data: null,
  error: null,
  edit: null, // scenario being edited: { forId, replace, builtin, scenario, problems }
  wedit: null, // workout being edited: { forId, replace, builtin, workout, problems }
  message: null,
  closedError: null, // why the trainer last closed, if it was an error: shown until dismissed or the next start
};

const TRAINER_VIEWS = ["today", "scenarios", "edit", "workout", "settings", "progress"];

async function loadTrainer() {
  try {
    ui.trainer.data = await invoke("trainer");
    ui.trainer.error = null;
    if (typeof settingsLoaded === "function") settingsLoaded();
  } catch (e) {
    ui.trainer.error = String(e);
  }
  if (TRAINER_VIEWS.includes(route().view)) render();
}

// What views that need the trainer's data pass to waitFor (app.js).
const trainerNeed = () => ({ key: "trainer", have: Boolean(ui.trainer.data), error: ui.trainer.error, load: loadTrainer });

// On the pages that start the trainer, after it closed with an error (a scenario it
// couldn't play, a crash), until it's dismissed or the trainer starts again.
function trainerErrorBanner() {
  const error = ui.trainer.closedError;
  return error ? `<section class="banner warn"><div><div class="banner-title">The trainer closed with an error</div><div class="banner-body">${esc(error)}</div></div>
    <button class="btn" type="button" data-action="dismiss-trainer-error">Dismiss</button></section>` : "";
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
  if (t.lifetime_s > 0) parts.push(`times out after ${num(t.lifetime_s)} s`);
  return parts.join(" · ");
}

// Adaptive ratings are in real units: seconds of target lifetime, degrees per second
// of target speed, or meters of target radius.
const higherIsHarder = (knob) => knob === "target_speed";
function fmtKnob(knob, v) {
  if (v === null || v === undefined) return "–";
  if (knob === "lifetime" || knob === "kill_pace") return `${v.toFixed(2)} s`;
  if (knob === "target_speed") return `${v.toFixed(1)} °/s`;
  return `${v.toFixed(3)} m`;
}
const knobWord = (knob) => ({ target_speed: "tracking speed", lifetime: "speed", kill_pace: "kill pace" })[knob] ?? "size";
const betterThan = (knob, a, b) => (higherIsHarder(knob) ? a > b : a < b);

const scenarioName = (id) => ui.trainer.data?.scenarios.find((e) => e.id === id)?.scenario?.name ?? id.replace(/^(builtin|user):/, "");
const minutes = (s) => `${Math.max(1, Math.round(s / 60))} min`;

// ---------- Scenario editor ----------

function edit(id) {
  const tr = ui.trainer;
  const wait = waitFor([trainerNeed()], "Loading…");
  if (wait) return wait;
  if (!tr.edit || tr.edit.forId !== id) {
    const entry = tr.data.scenarios.find((e) => e.id === id);
    let scenario;
    if (id === "new") scenario = structuredClone(tr.data.scenarios.find((e) => e.id === "builtin:timed-flicks")?.scenario);
    else scenario = structuredClone(entry?.scenario);
    if (!scenario) return `<div class="page">${head("Scenario editor", "Not found")}<div class="card note-body">That scenario couldn't be loaded. <a href="#scenarios">Back to Scenarios</a></div></div>`;
    if (id === "new") Object.assign(scenario, { name: "My scenario", author: "", description: "", tags: [] });
    else if (entry.builtin) Object.assign(scenario, { name: `${scenario.name} (my copy)`, author: "" });
    tr.edit = { forId: id, replace: entry && !entry.builtin ? id : null, builtin: !!entry?.builtin, scenario, problems: [] };
    // Decided once, here: a section that holds only defaults starts folded. After this only the
    // player's clicks change it, so typing in a field never folds the section under the cursor.
    editGroups().forEach((g, i) => (ui.folds[editFoldId(g)] = i === 0 || !holdsDefaults(g, scenario)));
  }
  const e = tr.edit;
  const groups = editGroups();
  return `<div class="page">
    ${head(e.replace ? "Edit scenario" : e.builtin ? "Copy of a built-in scenario" : "New scenario", esc(e.scenario.name || "Untitled"), `<span class="chips">
      <button class="btn" type="button" data-action="cancel-edit" data-href="${esc(e.replace || e.forId !== "new" ? scenarioHref(e.forId) : "#scenarios")}">Cancel</button>
      ${e.replace ? `<button class="btn" type="button" data-action="delete-scenario" data-id="${esc(e.replace)}">Delete</button>` : ""}
      <button class="btn" type="button" data-action="save-scenario">Save</button>
      <button class="btn primary" type="button" data-action="save-play">Save &amp; play adaptive</button></span>`)}
    ${trainerErrorBanner()}
    ${e.builtin ? `<section class="banner info"><div class="banner-body">Built-in scenarios stay as they are. Saving makes your own copy.</div></section>` : ""}
    <div id="edit-status">${editStatus()}</div>
    <div class="editor-groups">${groups.map((g, i) => editGroup(g, i === 0)).join("")}</div>
  </div>`;
}

// The editor's fields grouped as the file format does, in order.
function editGroups() {
  const groups = [];
  for (const f of ui.trainer.data.fields) {
    let g = groups.find((x) => x.name === f.group);
    if (!g) groups.push((g = { name: f.group, fields: [] }));
    g.fields.push(f);
  }
  return groups;
}

const editFoldId = (g) => `edit:${g.name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`;

// Whether every field of the group holds its default. Without the defaults (older preview
// data) nothing counts as default, so nothing folds.
function holdsDefaults(g, scenario) {
  const defaults = ui.trainer.data.defaults;
  if (!defaults) return false;
  return g.fields.every((f) => JSON.stringify(getPath(scenario, f.path)) === JSON.stringify(getPath(defaults, f.path)));
}

// The first group (the name) is always a plain card. The others are folds, open or not as decided when the
// editor opened the scenario. A folded group's summary says when it holds only defaults.
function editGroup(g, first) {
  const e = ui.trainer.edit;
  const form = `<div class="form-grid">${g.fields.map((f) => fieldInput(f, getPath(e.scenario, f.path))).join("")}</div>`;
  if (first) return `<section class="card"><h2>${esc(g.name)}</h2>${form}</section>`;
  return fold(editFoldId(g), g.name, `<section class="card">${form}</section>`, holdsDefaults(g, e.scenario) ? "defaults" : "");
}

function editStatus() {
  const e = ui.trainer.edit;
  if (!e) return "";
  const problems = e.problems.length
    ? `<section class="banner"><div><div class="banner-title">Fix before saving</div><ul class="problems">${e.problems.map((p) => `<li>${esc(p)}</li>`).join("")}</ul></div></section>`
    : "";
  let facts = "";
  try {
    facts = `<div class="small muted">${esc(scenarioFacts(e.scenario))}</div>`;
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
    case "toggle":
      control = `<span class="field"><input id="${id}" type="checkbox" data-path="${f.path}" data-kind="toggle" ${value ? "checked" : ""}></span>`;
      break;
    default:
      control = `<span class="muted">${esc(JSON.stringify(value))}</span>`;
  }
  return `<div class="form-row"><label for="${id}">${esc(f.label)}</label>${control}${help}</div>`;
}

// Whether the page on screen is the scenario editor, editing this draft. Answers
// that arrive after a wait only go on the page if it is: by then it can be another
// page (whose heading a late check used to overwrite), or another draft.
const editorShows = (draft) => route().view === "edit" && ui.trainer.edit === draft;

let checkTimer = null;
function recheck() {
  const e = ui.trainer.edit;
  clearTimeout(checkTimer);
  checkTimer = setTimeout(async () => {
    try {
      e.problems = await invoke("check_scenario", { scenario: e.scenario });
    } catch (err) {
      e.problems = [String(err)];
    }
    if (!editorShows(e)) return;
    const box = document.getElementById("edit-status");
    if (box) box.innerHTML = editStatus();
    const h1 = document.querySelector(".page-head h1");
    if (h1) h1.textContent = e.scenario.name || "Untitled";
  }, 150);
}

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
    case "toggle":
      v = el.checked;
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
  recheck();
}

async function saveScenario(thenPlay) {
  const e = ui.trainer.edit;
  try {
    const id = await invoke("save_scenario", { scenario: e.scenario, replace: e.replace });
    ui.trainer.edit = null;
    ui.trainer.message = `Saved ${e.scenario.name}.`;
    await loadTrainer();
    location.hash = scenarioHref(id);
    if (thenPlay) await startPlay(id, true);
  } catch (err) {
    e.problems = String(err).split("; ");
    if (!editorShows(e)) return;
    const box = document.getElementById("edit-status");
    if (box) box.innerHTML = editStatus();
    window.scrollTo(0, 0);
  }
}

// ---------- Workout editor ----------

function workout(id) {
  const tr = ui.trainer;
  const wait = waitFor([trainerNeed()], "Loading…");
  if (wait) return wait;
  if (!tr.wedit || tr.wedit.forId !== id) {
    const entry = tr.data.workouts.find((e) => e.id === id);
    let w = id === "new" ? { name: "My workout", description: "", author: "", items: [{ scenario: "builtin:timed-flicks", runs: 2, mode: "adaptive" }] } : structuredClone(entry?.workout);
    if (!w) return `<div class="page">${head("Workout", "Not found")}<div class="card note-body"><a href="#scenarios">Back to Scenarios</a></div></div>`;
    if (entry?.builtin) w.name = `${w.name} (my copy)`;
    tr.wedit = { forId: id, replace: entry && !entry.builtin ? id : null, builtin: !!entry?.builtin, workout: w, problems: [] };
  }
  const e = tr.wedit;
  const w = e.workout;
  const options = tr.data.scenarios.filter((s) => s.scenario).map((s) => [s.id, s.scenario.name]);
  const total = w.items.reduce((a, i) => {
    const s = tr.data.scenarios.find((x) => x.id === i.scenario)?.scenario;
    return a + (s ? s.timing.duration_s + s.timing.countdown_s : 60) * (i.runs || 1);
  }, 0);
  return `<div class="page">
    ${head(e.replace ? "Edit workout" : e.builtin ? "Copy of a built-in workout" : "New workout", esc(w.name || "Untitled"), `<span class="chips">
      <button class="btn" type="button" data-action="cancel-workout" data-href="${esc(e.forId !== "new" ? scenarioHref(`workout:${e.forId}`) : "#scenarios")}">Cancel</button>
      ${e.replace ? `<button class="btn" type="button" data-action="delete-workout" data-id="${esc(e.replace)}">Delete</button>` : ""}
      <button class="btn" type="button" data-action="save-workout">Save</button>
      <button class="btn primary" type="button" data-action="save-workout-play">Save &amp; start</button></span>`)}
    ${trainerErrorBanner()}
    ${e.problems.length ? `<section class="banner"><div><div class="banner-title">Fix before saving</div><ul class="problems">${e.problems.map((p) => `<li>${esc(p)}</li>`).join("")}</ul></div></section>` : ""}
    <section class="card">
      <div class="form-grid">
        <label class="form-row"><span>Name</span><span class="field"><input type="text" data-wfield="name" value="${esc(w.name)}"></span></label>
        <label class="form-row wide"><span>Description</span><textarea class="textarea" rows="2" data-wfield="description">${esc(w.description)}</textarea></label>
      </div>
    </section>
    <section class="card">
      <div class="card-head"><h2>Runs, in order</h2><span class="small muted">About ${minutes(total)} · ${w.items.reduce((a, i) => a + (i.runs || 1), 0)} runs</span></div>
      <div class="workout-items">${w.items.map((it, i) => `<div class="knob-row">
        <span class="small muted mono">${i + 1}</span>
        <select class="select" data-witem="${i}" data-part="scenario" aria-label="Scenario">${options.map(([v, l]) => `<option value="${esc(v)}" ${v === it.scenario ? "selected" : ""}>${esc(l)}</option>`).join("")}</select>
        <span class="field"><input type="number" min="1" max="20" step="1" data-witem="${i}" data-part="runs" value="${it.runs}" aria-label="Runs"><span class="muted small">runs</span></span>
        <select class="select" data-witem="${i}" data-part="mode" aria-label="Mode"><option value="adaptive" ${it.mode !== "standard" ? "selected" : ""}>Adaptive</option><option value="standard" ${it.mode === "standard" ? "selected" : ""}>Standard</option></select>
        <button class="icon-btn" type="button" data-action="witem-up" data-index="${i}" aria-label="Move up" ${i === 0 ? "disabled" : ""}>↑</button>
        <button class="icon-btn" type="button" data-action="witem-down" data-index="${i}" aria-label="Move down" ${i === w.items.length - 1 ? "disabled" : ""}>↓</button>
        <button class="icon-btn" type="button" data-action="witem-remove" data-index="${i}" aria-label="Remove">×</button>
      </div>`).join("")}</div>
      <div><button class="btn" type="button" data-action="witem-add">Add a scenario</button></div>
    </section>
  </div>`;
}

async function saveWorkout(thenPlay) {
  const e = ui.trainer.wedit;
  try {
    const id = await invoke("save_workout", { workout: e.workout, replace: e.replace });
    ui.trainer.wedit = null;
    ui.trainer.message = `Saved ${e.workout.name}.`;
    await loadTrainer();
    location.hash = scenarioHref(`workout:${id}`);
    if (thenPlay) await startWorkout(id);
  } catch (err) {
    e.problems = String(err).split("; ");
    render();
    window.scrollTo(0, 0);
  }
}

// ---------- Actions ----------

// Starts the trainer with start() (an invoke). If it can't start, that's the message;
// the last run's error banner goes either way.
async function launchTrainer(start) {
  ui.trainer.closedError = null;
  try {
    await start();
    ui.trainer.message = null;
  } catch (e) {
    ui.trainer.message = String(e);
  }
}

async function startPlay(id, adaptive) {
  await launchTrainer(() => invoke("play", { id, adaptive }));
  await loadTrainer();
}

async function startWorkout(id) {
  await launchTrainer(() => invoke("play_workout", { id }));
  await loadTrainer();
}

document.addEventListener("click", async (e) => {
  const btn = e.target.closest("[data-action]");
  if (!btn) return;
  const action = btn.dataset.action;
  const tr = ui.trainer;
  if (action === "play") startPlay(btn.dataset.id, btn.dataset.adaptive === "1");
  else if (action === "play-workout") startWorkout(btn.dataset.id);
  else if (action === "new-scenario") {
    tr.edit = null;
    location.hash = "#edit/new";
  } else if (action === "new-workout") {
    tr.wedit = null;
    location.hash = "#workout/new";
  } else if (action === "cancel-edit") {
    // Drop the draft, or the cancelled changes come back when the editor reopens.
    tr.edit = null;
    location.hash = btn.dataset.href;
  } else if (action === "cancel-workout") {
    tr.wedit = null;
    location.hash = btn.dataset.href;
  } else if (action === "dismiss-trainer-error") {
    tr.closedError = null;
    render();
  } else if (action === "save-scenario") saveScenario(false);
  else if (action === "save-play") saveScenario(true);
  else if (action === "save-workout") saveWorkout(false);
  else if (action === "save-workout-play") saveWorkout(true);
  else if (action.startsWith("witem-")) {
    const items = tr.wedit.workout.items;
    const i = Number(btn.dataset.index);
    if (action === "witem-add") items.push({ scenario: items.at(-1)?.scenario ?? "builtin:timed-flicks", runs: 1, mode: "adaptive" });
    else if (action === "witem-remove") items.splice(i, 1);
    else if (action === "witem-up" && i > 0) [items[i - 1], items[i]] = [items[i], items[i - 1]];
    else if (action === "witem-down" && i < items.length - 1) [items[i + 1], items[i]] = [items[i], items[i + 1]];
    render();
  } else if (action === "delete-scenario" || action === "delete-workout") {
    const id = btn.dataset.id;
    const isScenario = action === "delete-scenario";
    if (!confirm(`Delete ${id.replace(/^user:/, "")}? This removes the file; your past runs stay in your history.`)) return;
    try {
      await (isScenario ? invoke("delete_scenario", { id }) : invoke("delete_workout", { id }));
      tr.edit = null;
      tr.wedit = null;
      tr.message = `${isScenario ? "Scenario" : "Workout"} deleted.`;
    } catch (err) {
      tr.message = String(err);
    }
    location.hash = "#scenarios";
    loadTrainer();
  }
});

document.addEventListener("input", (e) => {
  const el = e.target;
  if (el.dataset.path) onEditorInput(el);
  else if (el.dataset.wfield) ui.trainer.wedit.workout[el.dataset.wfield] = el.value;
  else if (el.dataset.witem !== undefined) {
    const it = ui.trainer.wedit.workout.items[Number(el.dataset.witem)];
    it[el.dataset.part] = el.dataset.part === "runs" ? Number(el.value) : el.value;
  }
});

document.addEventListener("change", (e) => {
  if (e.target.dataset.witem !== undefined && route().view === "workout") render();
});

// V or I on the trainer's results screen closes the trainer and asks for a page here.
function openAfterTrainer(open) {
  if (open?.replay != null) location.hash = `#replay/${open.replay}`;
  else if (open?.scenario) location.hash = scenarioHrefByName(open.scenario);
}

// The trainer saved its runs and closed: reload what shows them. The event says
// whether it closed with an error ({ error: string | null }) for the banner, and
// which page the player asked for ({ replay: id } or { scenario: name }, or null).
listen("trainer-closed", async (event) => {
  ui.trainer.closedError = event?.payload?.error ?? null;
  await load();
  await loadTrainer();
  if (typeof loadInsightsOverview === "function") loadInsightsOverview(true);
  // Imported runs show on the KovaaK's scenario's page.
  if (typeof ui.sceInfo !== "undefined") ui.sceInfo.clear();
  openAfterTrainer(event?.payload?.open);
});
