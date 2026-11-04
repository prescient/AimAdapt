// Play screen, workouts, and the scenario and workout editors for the built-in
// trainer. Loaded after app.js and uses its helpers (esc, num, head, invoke, ui,
// render, route, fmtShort, dt).
//
// The scenario editor's form is built from `trainer.fields`, the engine's own list of
// every scenario setting with its limits and help text, so new engine options show up
// here without UI changes.

ui.trainer = {
  data: null,
  error: null,
  filter: "all",
  edit: null, // scenario being edited: { forId, replace, builtin, scenario, problems }
  wedit: null, // workout being edited: { forId, replace, builtin, workout, problems }
  message: null,
};

const CATEGORIES = [
  ["all", "All"],
  ["clicking", "Clicking"],
  ["tracking", "Tracking"],
  ["switching", "Switching"],
  ["mine", "Yours"],
];

const TRAINER_VIEWS = ["play", "edit", "workout", "settings"];

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
  if (knob === "lifetime") return `${v.toFixed(2)} s`;
  if (knob === "target_speed") return `${Math.round(v)} °/s`;
  return `${v.toFixed(2)} m`;
}
const knobWord = (knob) => (knob === "target_speed" ? "tracking speed" : knob === "lifetime" ? "speed" : "size");
const betterThan = (knob, a, b) => (higherIsHarder(knob) ? a > b : a < b);

const scenarioName = (id) => ui.trainer.data?.scenarios.find((e) => e.id === id)?.scenario?.name ?? id.replace(/^(builtin|user):/, "");
const minutes = (s) => `${Math.max(1, Math.round(s / 60))} min`;

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
    ${head("Built-in aim trainer", "Play", `<span class="chips">
      <button class="btn" type="button" data-action="new-workout">New workout</button>
      <button class="btn primary" type="button" data-action="new-scenario">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"></path></svg>New scenario</button></span>`)}
    ${d.playing ? `<section class="banner info"><div><div class="banner-title">The trainer is open</div><div class="banner-body">Finish or press Esc to come back. Each run is saved when its timer ends.</div></div></section>` : ""}
    ${tr.message ? `<section class="banner"><div class="banner-body">${esc(tr.message)}</div></section>` : ""}
    ${profileBar()}
    <section class="banner info"><div><div class="banner-title">Adaptive mode keeps you at about 85%</div>
      <div class="banner-body">Adaptive mode rates you in real units: the target lifetime (or tracking speed) you hit 85% of the time. It finds that in the first 15–25 targets, then mixes targets near your rating with harder stretch targets and a few easy ones (change the mix in Settings). Hitting a stretch target faster than ever is a breakthrough. Standard mode plays the scenario exactly as written.</div></div></section>
    ${workoutsCard()}
    <div class="chips" role="group" aria-label="Filter scenarios">
      ${CATEGORIES.map(([k, label]) => `<button class="chip ${tr.filter === k ? "on" : ""}" type="button" data-action="trainer-filter" data-filter="${k}" aria-pressed="${tr.filter === k}">${label}</button>`).join("")}
    </div>
    <div class="grid">${list.map(scenarioCard).join("") || `<div class="card note-body">No scenarios here yet. Use New scenario to make one.</div>`}</div>
    <div class="small muted">Your scenarios and workouts are TOML files in <span class="mono">${esc(d.scenarios_dir)}</span> and <span class="mono">${esc(d.workouts_dir)}</span>. See SCENARIOS.md for every setting.</div>
  </div>`;
}

function workoutsCard() {
  const d = ui.trainer.data;
  return `<section class="card">
    <div class="card-head"><h2>Workouts</h2><span class="small muted">Several scenarios back to back in one session</span></div>
    <div class="workout-list">${d.workouts.map((e) => {
      const w = e.workout;
      const id = esc(e.id);
      if (!w) return `<div class="workout-row"><div class="main"><div class="title">${esc(e.id)}</div><div class="sub error">${esc(e.error)}</div></div>
        <button class="btn" type="button" data-action="delete-workout" data-id="${id}">Delete file</button></div>`;
      const steps = w.items.map((i) => `${esc(scenarioName(i.scenario))}${i.runs > 1 ? ` ×${i.runs}` : ""}${i.mode === "standard" ? " (standard)" : ""}`).join(", ");
      return `<div class="workout-row">
        <div class="main"><div class="title">${esc(w.name)} ${e.builtin ? "" : '<span class="tag gold">Yours</span>'} <span class="small muted">${minutes(e.seconds)} · ${w.items.reduce((a, i) => a + i.runs, 0)} runs</span></div>
          <div class="sub">${esc(w.description)}</div><div class="sub">${steps}</div></div>
        <span class="chips">
          <button class="btn primary" type="button" data-action="play-workout" data-id="${id}" ${d.playing ? "disabled" : ""}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4l13 8-13 8z"></path></svg>Start</button>
          <a class="btn" href="#workout/${encodeURIComponent(e.id)}">${e.builtin ? "Copy &amp; edit" : "Edit"}</a></span>
      </div>`;
    }).join("")}</div>
  </section>`;
}

function scenarioCard(e) {
  const s = e.scenario;
  const id = esc(e.id);
  if (!s) {
    return `<section class="card"><div class="card-head"><h3>${esc(e.id.replace(/^user:/, ""))}</h3><span class="tag grey">Can't load</span></div>
      <div class="note-body error">${esc(e.error)}</div>
      <div class="chips"><button class="btn" type="button" data-action="delete-scenario" data-id="${id}">Delete file</button></div></section>`;
  }
  const playing = ui.trainer.data.playing ? "disabled" : "";
  const level = e.rating !== null && e.rating !== undefined
    ? `85% ${knobWord(e.knob)} <span class="num">${esc(fmtKnob(e.knob, e.rating))}</span>${e.best_rating !== null && betterThan(e.knob, e.best_rating, e.rating) ? ` <span class="muted">(best ${esc(fmtKnob(e.knob, e.best_rating))})</span>` : ""}${e.fastest !== null ? ` · fastest hit ${esc(fmtKnob(e.knob, e.fastest))}` : ""}`
    : '<span class="muted">Not rated yet</span>';
  const standard = e.runs ? ` · best score <span class="num">${num(e.best)}</span>` : "";
  return `<section class="card scenario-card">
    <div class="card-head"><h3>${esc(s.name)}</h3><span>${e.builtin ? "" : '<span class="tag gold">Yours</span> '}<span class="tag grey">${esc(s.category)}</span></span></div>
    <div class="note-body">${esc(s.description || "No description.")}</div>
    <div class="small muted">${esc(scenarioFacts(s))}</div>
    <div class="small">${level}${standard}</div>
    <div class="chips">
      <button class="btn primary" type="button" data-action="play" data-id="${id}" data-adaptive="1" ${playing}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4l13 8-13 8z"></path></svg>Adaptive</button>
      <button class="btn" type="button" data-action="play" data-id="${id}" data-adaptive="0" ${playing}>Standard</button>
      <a class="btn" href="#edit/${encodeURIComponent(e.id)}">${e.builtin ? "Copy &amp; edit" : "Edit"}</a>
      ${e.runs || e.adaptive_runs ? `<a class="btn" href="#insights/${encodeURIComponent(s.name)}">Insights</a>` : ""}
    </div>
  </section>`;
}

// ---------- Scenario editor ----------

function edit(id) {
  const tr = ui.trainer;
  if (!tr.data) {
    loadTrainer();
    return `<div class="empty-state">Loading…</div>`;
  }
  if (!tr.edit || tr.edit.forId !== id) {
    const entry = tr.data.scenarios.find((e) => e.id === id);
    let scenario;
    if (id === "new") scenario = structuredClone(tr.data.scenarios.find((e) => e.id === "builtin:timed-flicks")?.scenario);
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
      <button class="btn primary" type="button" data-action="save-play">Save &amp; play adaptive</button></span>`)}
    ${e.builtin ? `<section class="banner info"><div class="banner-body">Built-in scenarios stay as they are. Saving makes your own copy.</div></section>` : ""}
    <div id="edit-status">${editStatus()}</div>
    <div class="editor-groups">${groups.map((g) => `<section class="card"><h2>${esc(g.name)}</h2><div class="form-grid">${g.fields.map((f) => fieldInput(f, getPath(e.scenario, f.path))).join("")}</div></section>`).join("")}</div>
  </div>`;
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

function knobsInput(f, value) {
  const rows = (value || []).map((k, i) => `<div class="knob-row">
      <select class="select" data-knob="${i}" data-part="knob" aria-label="Setting">${f.options.map((o) => `<option value="${o.value}" ${o.value === k.knob ? "selected" : ""} title="${esc(o.help)}">${esc(o.label)}</option>`).join("")}</select>
      <span class="field"><input type="number" step="any" data-knob="${i}" data-part="easy" value="${esc(k.easy)}" aria-label="Easy value"><span class="muted small">easy</span></span>
      <span class="field"><input type="number" step="any" data-knob="${i}" data-part="hard" value="${esc(k.hard)}" aria-label="Hard value"><span class="muted small">hard</span></span>
      <button class="icon-btn" type="button" data-action="remove-knob" data-index="${i}" aria-label="Remove">×</button>
    </div>`).join("");
  return `<div class="knobs">${rows || '<div class="small muted">Using the default for this kind of scenario.</div>'}
    <button class="btn" type="button" data-action="add-knob">Add a setting</button></div>`;
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
    case "knobs":
      return `<div class="form-row wide"><label>${esc(f.label)}</label>${knobsInput(f, value)}${help}</div>`;
    default:
      control = `<span class="muted">${esc(JSON.stringify(value))}</span>`;
  }
  return `<div class="form-row"><label for="${id}">${esc(f.label)}</label>${control}${help}</div>`;
}

let checkTimer = null;
function recheck() {
  const e = ui.trainer.edit;
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

function onEditorInput(el) {
  const e = ui.trainer.edit;
  if (!e) return;
  if (el.dataset.knob !== undefined) {
    const k = e.scenario.adaptive.knobs[Number(el.dataset.knob)];
    k[el.dataset.part] = el.dataset.part === "knob" ? el.value : Number(el.value);
    recheck();
    return;
  }
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
  recheck();
}

async function saveScenario(thenPlay) {
  const e = ui.trainer.edit;
  try {
    const id = await invoke("save_scenario", { scenario: e.scenario, replace: e.replace });
    ui.trainer.edit = null;
    ui.trainer.message = `Saved ${e.scenario.name}.`;
    await loadTrainer();
    location.hash = "#play";
    if (thenPlay) await startPlay(id, true);
  } catch (err) {
    e.problems = String(err).split("; ");
    document.getElementById("edit-status").innerHTML = editStatus();
    window.scrollTo(0, 0);
  }
}

// ---------- Workout editor ----------

function workout(id) {
  const tr = ui.trainer;
  if (!tr.data) {
    loadTrainer();
    return `<div class="empty-state">Loading…</div>`;
  }
  if (!tr.wedit || tr.wedit.forId !== id) {
    const entry = tr.data.workouts.find((e) => e.id === id);
    let w = id === "new" ? { name: "My workout", description: "", author: "", items: [{ scenario: "builtin:timed-flicks", runs: 2, mode: "adaptive" }] } : structuredClone(entry?.workout);
    if (!w) return `<div class="page">${head("Workout", "Not found")}<div class="card note-body"><a href="#play">Back to Play</a></div></div>`;
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
      <a class="btn" href="#play">Cancel</a>
      ${e.replace ? `<button class="btn" type="button" data-action="delete-workout" data-id="${esc(e.replace)}">Delete</button>` : ""}
      <button class="btn" type="button" data-action="save-workout">Save</button>
      <button class="btn primary" type="button" data-action="save-workout-play">Save &amp; start</button></span>`)}
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
    location.hash = "#play";
    if (thenPlay) await startWorkout(id);
  } catch (err) {
    e.problems = String(err).split("; ");
    render();
    window.scrollTo(0, 0);
  }
}

// ---------- Actions ----------

async function startPlay(id, adaptive) {
  try {
    await invoke("play", { id, adaptive });
    ui.trainer.message = null;
  } catch (e) {
    ui.trainer.message = String(e);
  }
  await loadTrainer();
}

async function startWorkout(id) {
  try {
    await invoke("play_workout", { id });
    ui.trainer.message = null;
  } catch (e) {
    ui.trainer.message = String(e);
  }
  await loadTrainer();
}

document.addEventListener("click", async (e) => {
  const btn = e.target.closest("[data-action]");
  if (!btn) return;
  const action = btn.dataset.action;
  const tr = ui.trainer;
  if (action === "trainer-filter") {
    tr.filter = btn.dataset.filter;
    render();
  } else if (action === "play") startPlay(btn.dataset.id, btn.dataset.adaptive === "1");
  else if (action === "play-workout") startWorkout(btn.dataset.id);
  else if (action === "new-scenario") location.hash = "#edit/new";
  else if (action === "new-workout") location.hash = "#workout/new";
  else if (action === "save-scenario") saveScenario(false);
  else if (action === "save-play") saveScenario(true);
  else if (action === "save-workout") saveWorkout(false);
  else if (action === "save-workout-play") saveWorkout(true);
  else if (action === "add-knob") {
    const ks = tr.edit.scenario.adaptive.knobs;
    const used = ks.map((k) => k.knob);
    const opts = tr.data.fields.find((f) => f.kind === "knobs").options.map((o) => o.value);
    const knob = opts.find((o) => !used.includes(o)) ?? opts[0];
    const cur = { lifetime: tr.edit.scenario.targets.lifetime_s || 1.2, radius: tr.edit.scenario.targets.radius, speed: tr.edit.scenario.targets.movement.speed[1], change_every: tr.edit.scenario.targets.movement.change_every_s[1], spread: 1, health: tr.edit.scenario.targets.health, respawn_delay: tr.edit.scenario.targets.respawn_delay_s || 0.2 }[knob];
    ks.push({ knob, easy: Math.round(cur * 150) / 100, hard: Math.round(cur * 50) / 100 });
    render();
    recheck();
  } else if (action === "remove-knob") {
    tr.edit.scenario.adaptive.knobs.splice(Number(btn.dataset.index), 1);
    render();
    recheck();
  } else if (action.startsWith("witem-")) {
    const items = tr.wedit.workout.items;
    const i = Number(btn.dataset.index);
    if (action === "witem-add") items.push({ scenario: items.at(-1)?.scenario ?? "builtin:timed-flicks", runs: 1, mode: "adaptive" });
    else if (action === "witem-remove") items.splice(i, 1);
    else if (action === "witem-up" && i > 0) [items[i - 1], items[i]] = [items[i], items[i - 1]];
    else if (action === "witem-down" && i < items.length - 1) [items[i + 1], items[i]] = [items[i], items[i + 1]];
    render();
  } else if (action === "delete-scenario" || action === "delete-workout") {
    const id = btn.dataset.id;
    const kind = action === "delete-scenario" ? "scenario" : "workout";
    if (!confirm(`Delete ${id.replace(/^user:/, "")}? This removes the file; your past runs stay in your history.`)) return;
    try {
      await invoke(action === "delete-scenario" ? "delete_scenario" : "delete_workout", { id });
      tr.edit = null;
      tr.wedit = null;
      tr.message = `${kind === "scenario" ? "Scenario" : "Workout"} deleted.`;
    } catch (err) {
      tr.message = String(err);
    }
    location.hash = "#play";
    loadTrainer();
  }
});

document.addEventListener("input", (e) => {
  const el = e.target;
  if (el.dataset.path || el.dataset.knob !== undefined) onEditorInput(el);
  else if (el.dataset.wfield) ui.trainer.wedit.workout[el.dataset.wfield] = el.value;
  else if (el.dataset.witem !== undefined) {
    const it = ui.trainer.wedit.workout.items[Number(el.dataset.witem)];
    it[el.dataset.part] = el.dataset.part === "runs" ? Number(el.value) : el.value;
  }
});

document.addEventListener("change", (e) => {
  if (e.target.dataset.witem !== undefined && route().view === "workout") render();
});

if (tauri) {
  tauri.event.listen("trainer-closed", async () => {
    await load();
    await loadTrainer();
    if (typeof loadInsightsOverview === "function") loadInsightsOverview(true);
  });
}
