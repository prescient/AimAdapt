// AimAdapt UI. Plain JS, no build step: it asks the Rust side for a snapshot of
// everything (`snapshot`), renders the current screen into <main>, and reloads
// when the folder watcher reports new runs.

// ---------- talking to the Rust side ----------
//
// Views only call invoke() and listen(); this is the one place that knows whether
// it's running in the app. Outside Tauri (a browser preview), commands are answered
// from a dump made from a test database with
//   aimadapt-app --db <file> --dump-snapshot crates/aimadapt-app/dev/dev-snapshot.json
// The preview server serves crates/aimadapt-app, so this page is /ui/index.html and
// the dump is at ../dev/ (never in ui/, which is built into the exe).

const tauri = window.__TAURI__;
const devData = tauri
  ? null
  : fetch("../dev/dev-snapshot.json").then((r) => {
      if (!r.ok) throw new Error(`No preview data: ../dev/dev-snapshot.json gave ${r.status}`);
      return r.json();
    });

// What the preview answers for each command, from the dump (d) and the call's args (a).
const PREVIEW = {
  snapshot: (d) => d.snapshot,
  scenario: (d, a) => d.scenarios[a.name] ?? null,
  rescan: (d) => d.snapshot.last_import,
  trainer: (d) => d.trainer,
  check_scenario: () => [],
  check_profiles: () => [[], []],
  save_marks: (d, a) => a.marks,
  insights_overview: (d) => ({ scenarios: d.insights_overview }),
  insights: (d, a) => d.insights?.[a.scenario] ?? null,
  replay: (d, a) => d.replays?.[a.runId] ?? null,
  program: (d) => d.program,
  sce_library: (d) => d.sce_library ?? [],
  sce_info: (d, a) => d.sce_info?.[a.name] ?? { path: null, playable: false, reasons: [], notes: [], runs: 0, best: null, last_played: null },
};

const invoke = tauri
  ? (cmd, args) => tauri.core.invoke(cmd, args)
  : async (cmd, args) => {
      if (!Object.hasOwn(PREVIEW, cmd)) throw new Error("Only available in the app");
      return PREVIEW[cmd](await devData, args);
    };

// Events from the Rust side (new runs, the trainer closing). The preview has none.
const listen = tauri ? (event, handler) => tauri.event.listen(event, handler) : () => {};

const ui = {
  snap: null,
  error: null,
  scenarioCache: new Map(), // a KovaaK's scenario's history by name (see scenarioDetail)
  calMonth: null, // Date for the first of the month shown on Progress › Time
  sessionsShown: 20,
  folds: {}, // which folds (see fold()) are open, by id, so a redraw keeps them as they were
  allRuns: false, // a scenario's Runs table shows every run, not the latest 30
  rescanning: false,
  rescanError: null, // why the last Rescan failed, shown by the button until the next one
};

// ---------- formatting ----------

const esc = (s) =>
  String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

function dur(seconds) {
  const m = Math.round((seconds || 0) / 60);
  if (m < 60) return `${m}m`;
  return `${Math.floor(m / 60)}h ${String(m % 60).padStart(2, "0")}m`;
}

function num(v) {
  if (v === null || v === undefined || Number.isNaN(v)) return "–";
  if (Math.abs(v) >= 1000) return Math.round(v).toLocaleString("en-US");
  return Number.isInteger(v) ? String(v) : v.toFixed(1);
}

const pct = (v, digits = 0) => (v === null || v === undefined ? "–" : `${v > 0 ? "+" : ""}${v.toFixed(digits)}%`);
const acc = (v) => (v === null || v === undefined ? "–" : `${(v * 100).toFixed(1)}%`);
const dt = (s) => new Date(s); // NaiveDateTime strings are local time
const day = (s) => {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(y, m - 1, d);
};
const dayKey = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const addDays = (d, n) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
const fmtDay = (d) => d.toLocaleDateString("en-US", { weekday: "short", month: "short", day: "numeric" });
const fmtShort = (d) => d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
const fmtTime = (d) => d.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
const sessionLength = (s) => (dt(s.end) - dt(s.start)) / 1000;
const median = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  return s.length ? s[Math.floor(s.length / 2)] : null;
};

function sensLabel(s) {
  const sens = s.horiz_sens !== null ? `${s.horiz_sens} ${s.sens_scale ?? ""}`.trim() : "sens ?";
  return `${sens} · ${s.dpi ?? "?"} DPI · FOV ${s.fov ?? "?"}`;
}

// ---------- data ----------

async function load() {
  try {
    ui.snap = await invoke("snapshot");
    ui.error = null;
    ui.scenarioCache.clear();
  } catch (e) {
    ui.error = String(e);
  }
  render();
}

// ---------- loading what a view needs ----------
//
// Views draw from data that loads in the background and draw again when it comes
// in, so they ask for it on every render while it's missing. A load mustn't start
// again while it's running, or after it failed: a failed load that each render
// asked for again was an endless load/fail/render loop that never showed its error.

const running = new Set();

// Runs fetchData() unless the load under this key is still running.
function startOnce(key, fetchData) {
  if (running.has(key)) return;
  running.add(key);
  Promise.resolve()
    .then(fetchData)
    .catch((e) => console.error(e))
    .finally(() => running.delete(key));
}

// What a view shows until it has everything in `needs`: null once it has, else the
// first failed load's error, else `text` while the rest load (each started once).
// A need is { key, have, error, load }: see trainerNeed, programNeed, overviewNeed.
function waitFor(needs, text) {
  const missing = needs.filter((n) => !n.have);
  if (!missing.length) return null;
  const failed = missing.find((n) => n.error);
  if (failed) return `<div class="card error">${esc(failed.error)}</div>`;
  for (const n of missing) startOnce(n.key, n.load);
  return `<div class="empty-state">${esc(text)}</div>`;
}

// Marks a cache entry whose load is running.
class Loading {}

// One item's data in `cache`, loaded once by get(): undefined until it's in, then
// the value, or { error } when the load failed, for the page to show rather than
// ask again. Scenarios draws again when it arrives. If the cache is cleared while a
// load runs, that load's answer is dropped and the next render starts a fresh one.
function loadCached(cache, key, get) {
  if (!cache.has(key)) {
    const mark = new Loading();
    cache.set(key, mark);
    get()
      .catch((e) => ({ error: String(e) }))
      .then((value) => {
        if (cache.get(key) !== mark) return;
        cache.set(key, value ?? null);
        if (route().view === "scenarios") render();
      });
  }
  const v = cache.get(key);
  return v instanceof Loading ? undefined : v;
}

// A KovaaK's scenario's history from its stats files: undefined while it loads, null
// if there's none, or { error }.
const scenarioDetail = (name) => loadCached(ui.scenarioCache, name, () => invoke("scenario", { name }));

async function rescan() {
  ui.rescanning = true;
  ui.rescanError = null;
  render();
  try {
    await invoke("rescan");
  } catch (e) {
    // Not in ui.error: the reload below clears that, and it only shows without data.
    ui.rescanError = String(e);
  }
  ui.rescanning = false;
  await load();
}

// ---------- routing ----------

// Pages that moved when ten pages became four. Old links (and the help guide's
// history) still land in the right place.
const MOVED = {
  dashboard: "today",
  play: "scenarios",
  program: "progress/skills",
  training: "progress/time",
  benchmarks: "progress/benchmarks",
  library: "settings/kovaaks",
};
const PROGRESS_TABS = [["skills", "Skills"], ["benchmarks", "Benchmarks from KovaaK's"], ["time", "Time"]];
const SETTINGS_TABS = [["aim", "Aim and display"], ["goals", "Goals and time"], ["kovaaks", "KovaaK's"], ["data", "Your data"]];

// The tabs of a scenario's own page. Overview is also the bare address.
const ITEM_TABS = [["overview", "Overview"], ["insights", "Insights"], ["runs", "Runs"]];

// A scenario's page can end in a tab: #scenarios/<key>/insights. The key is written
// with encodeURIComponent, so a "/" inside it is %2F and the tab is the last segment.
// Every other address is the view and an argument, as before.
function route() {
  const [view, ...rest] = (location.hash.slice(1) || "today").split("/");
  if (view === "scenarios" && rest.length >= 2 && ITEM_TABS.some(([id]) => id === rest.at(-1))) {
    const tab = rest.pop();
    return { view, arg: decodeURIComponent(rest.join("/")), tab };
  }
  return { view, arg: rest.length ? decodeURIComponent(rest.join("/")) : null };
}

// Where an old address now lives, or null if it's current.
function movedTo({ view, arg }) {
  if (MOVED[view]) return `#${MOVED[view]}`;
  if (view === "insights") return arg ? `#scenarios/${encodeURIComponent(`name:${arg}`)}` : "#scenarios";
  // #progress/<scenario> was a scenario's history; it's on Scenarios now.
  if (view === "progress" && arg && !PROGRESS_TABS.some(([id]) => id === arg)) return `#scenarios/${encodeURIComponent(`name:${arg}`)}`;
  return null;
}

// The sidebar entry each page belongs to.
const NAV_OF = { edit: "scenarios", workout: "scenarios", replay: "scenarios" };

function render() {
  const r = route();
  const moved = movedTo(r);
  if (moved) {
    location.replace(moved);
    return;
  }
  const { view, arg, tab } = r;
  const navView = NAV_OF[view] ?? view;
  document.querySelectorAll(".nav").forEach((a) => {
    const on = a.dataset.view === navView;
    a.classList.toggle("active", on);
    on ? a.setAttribute("aria-current", "page") : a.removeAttribute("aria-current");
  });
  renderStatus();
  const main = document.getElementById("main");
  if (ui.error && !ui.snap) {
    main.innerHTML = `<div class="empty-state error">Couldn't load data: ${esc(ui.error)}</div>`;
    return;
  }
  if (!ui.snap) return;
  const views = { today, scenarios: scenariosPage, edit, workout, progress: progressPage, settings: settingsPage, help: helpPage, replay: replayPage };
  const focused = focusedControl(main);
  try {
    main.innerHTML = (views[view] || today)(arg, tab);
  } catch (e) {
    // Say what broke instead of leaving the last page up (or "Loading…") with no word.
    console.error(e);
    main.innerHTML = `<div class="empty-state error">This page couldn't be drawn: ${esc(e?.message ?? e)}</div>`;
    return;
  }
  refocus(main, focused);
  if (view === "replay") afterReplay();
}

// ---------- keeping the keyboard focus ----------
//
// Every render replaces <main>, which drops the focus. It goes back to the same
// control when the new page still has one: matched by id, or else by its tag, link
// and data-* attributes (data-action with the data-id, data-key, data-index… that
// say what it acts on), with a text field's caret where it was.

function controlKey(el) {
  if (el.id) return `#${el.id}`;
  const attrs = [...el.attributes].filter((a) => a.name === "href" || a.name.startsWith("data-"));
  return attrs.length ? `${el.tagName} ${attrs.map((a) => `${a.name}=${a.value}`).sort().join(" ")}` : null;
}

function focusedControl(root) {
  const el = document.activeElement;
  if (!el || el === root || !root.contains(el)) return null;
  const key = controlKey(el);
  if (!key) return null;
  let caret = null;
  try {
    if (typeof el.selectionStart === "number") caret = [el.selectionStart, el.selectionEnd];
  } catch {
    // Not a text field.
  }
  return { key, tag: el.tagName, caret };
}

function refocus(root, focused) {
  if (!focused) return;
  const el = [...root.querySelectorAll(focused.tag)].find((x) => controlKey(x) === focused.key);
  if (!el) return;
  el.focus({ preventScroll: true });
  if (focused.caret) {
    try {
      el.setSelectionRange(...focused.caret);
    } catch {
      // Number fields have no caret to set.
    }
  }
}

function renderStatus() {
  const s = ui.snap;
  const el = document.getElementById("status");
  if (!s) return;
  const watching = !!s.stats_dir;
  const last = s.training.runs.at(-1);
  el.innerHTML = `
    <div class="status-title"><span class="dot ${watching ? "" : "off"}"></span>${watching ? "KovaaK's linked" : "KovaaK's not found"}</div>
    <div class="small muted">${last ? `Last run ${esc(fmtShort(dt(last.finished_at)))}, ${esc(fmtTime(dt(last.finished_at)))}` : "No runs yet"}</div>`;
}

// ---------- shared pieces ----------

const kpi = (label, value, note = "") =>
  `<div class="kpi"><div class="kpi-label">${label}</div><div class="kpi-value">${value}</div>${note ? `<div class="kpi-note">${note}</div>` : ""}</div>`;

function head(eyebrow, title, actions = "") {
  const link = typeof helpLink === "function" ? helpLink() : "";
  return `<header class="page-head"><div><div class="eyebrow">${eyebrow}</div><h1>${title}</h1></div><div class="head-actions">${actions}${link}</div></header>`;
}

// Rescan, with why the last one failed beside it.
const rescanButton = () =>
  `<span class="rescan">${ui.rescanError ? `<span class="error small">Rescan failed: ${esc(ui.rescanError)}</span>` : ""}
    <button class="btn" type="button" data-action="rescan" ${ui.rescanning ? "disabled" : ""}>
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.3 5.7"></path><path d="M20 4v7h-7"></path></svg>
    ${ui.rescanning ? "Scanning…" : "Rescan folder"}</button></span>`;

// Tabs that are links, so each tab has its own address and Back works. The tab named
// `bare` has the base address with no suffix (an item's Overview is the address every
// list links to).
function tabs(base, list, current, bare = null) {
  const href = (id) => `#${base}${id === bare ? "" : `/${id}`}`;
  return `<nav class="tabs" aria-label="Sections">${list.map(([id, label]) => `<a class="tab ${id === current ? "on" : ""}" href="${href(id)}" ${id === current ? 'aria-current="page"' : ""}>${label}</a>`).join("")}</nav>`;
}

// A section the reader opens when they want it: a native <details>, so the keyboard
// works. Whether it's open is kept in ui.folds, because every render redraws it. The
// summary has an id, so the focus stays on it across a render (see refocus).
function fold(id, title, body) {
  return `<details class="fold" data-fold="${esc(id)}" ${ui.folds[id] ? "open" : ""}>
    <summary id="fold-${esc(id)}">${esc(title)}</summary><div class="fold-body">${body}</div></details>`;
}

// toggle doesn't bubble, so listen on the way down.
document.addEventListener("toggle", (e) => {
  const id = e.target.dataset?.fold;
  if (id) ui.folds[id] = e.target.open;
}, true);

const noRunsYet = () =>
  `<div class="card"><div class="note-body">No runs yet. Play something on <a href="#scenarios">Scenarios</a>${ui.snap.stats_dir ? ", or in KovaaK's," : ""} and it shows up here.</div></div>`;

function totals() {
  const t = ui.snap.training;
  const active = t.sessions.reduce((a, s) => a + s.active_s, 0);
  const session = t.sessions.reduce((a, s) => a + sessionLength(s), 0);
  const first = day(t.days[0].day);
  const today = day(t.today);
  const calendarDays = Math.round((today - first) / 86400000) + 1;
  return { active, session, first, today, calendarDays };
}

// ---------- Progress ----------

function progressPage(arg) {
  const tab = PROGRESS_TABS.some(([id]) => id === arg) ? arg : "skills";
  const body = { skills: skillsTab, benchmarks: benchmarksTab, time: timeTab }[tab]();
  return `<div class="page">${head("Across all your scenarios", "Progress")}${tabs("progress", PROGRESS_TABS, tab)}${body}</div>`;
}

function dailyChart(n) {
  const s = ui.snap;
  const t = s.training;
  const today = day(t.today);
  const goal = s.time_settings.daily_goal_min * 60;
  const byDay = new Map(t.days.map((d) => [d.day, d]));
  const first = day(t.days[0].day);
  const days = Array.from({ length: n }, (_, i) => addDays(today, i - n + 1));
  const max = Math.max(goal * 1.2, ...days.map((d) => byDay.get(dayKey(d))?.session_s ?? 0)) * 1.05;
  // The bar area sits between the 24px value row and the 24px label row.
  const goalFrac = goal / max;
  const cols = days.map((d) => {
    const stat = byDay.get(dayKey(d));
    const isToday = dayKey(d) === t.today;
    const label = d.toLocaleDateString("en-US", { weekday: "short", day: "numeric" });
    if (!stat) {
      const note = isToday ? "today" : d < first ? "–" : "0m";
      return `<div class="bar-col ${isToday ? "today" : ""}"><div class="bar-val">${note}</div><div class="bar-area"></div><div class="bar-label">${label}</div></div>`;
    }
    return `<div class="bar-col ${isToday ? "today" : ""}"><div class="bar-val">${dur(stat.active_s)}</div>
      <div class="bar-area" title="${dur(stat.active_s)} active, ${dur(stat.session_s)} session, ${stat.runs} runs">
        <div class="bar" style="height:${((stat.active_s / max) * 100).toFixed(1)}%;background:var(--rust)"></div>
        <div class="bar" style="height:${((stat.session_s / max) * 100).toFixed(1)}%;background:var(--session)"></div></div>
      <div class="bar-label">${label}</div></div>`;
  });
  return `<div class="card-head"><h2>Daily time</h2><div class="legend">
      <span><i class="swatch" style="background:var(--rust)"></i>Active</span>
      <span><i class="swatch" style="background:var(--session)"></i>Session</span>
      <span><i style="width:14px;border-top:2px dashed var(--dim)"></i>${s.time_settings.daily_goal_min} min goal</span></div></div>
    <div class="bars"><div class="goal" style="bottom:calc(24px + (100% - 48px) * ${goalFrac.toFixed(4)})"></div>${cols.join("")}</div>`;
}

// A KovaaK's scenario's score history: the figures and the score chart.
function scoreHistoryCard({ stats: s, history }) {
  const periods = ui.snap.sens_periods;
  const counted = history.filter((r) => !r.restarted && r.score !== null);
  const sessionsCount = new Set(counted.map((r) => r.day)).size;
  return `<section class="card" aria-label="Score history" style="gap:18px">
      <div>
        <div class="small muted">${s.runs} runs${s.runs !== s.counted_runs ? ` (${s.runs - s.counted_runs} restarted)` : ""} · ${sessionsCount} ${sessionsCount === 1 ? "day" : "days"} · first played ${esc(fmtShort(dt(s.first_played)))}</div></div>
      <div class="kpis" style="grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:12px">
        <div class="tile"><div class="kpi-label">Personal best</div><div class="kpi-value" style="color:var(--gold)">${num(s.best_score)}</div><div class="small muted">${s.best_at ? esc(fmtShort(dt(s.best_at)) + ", " + fmtTime(dt(s.best_at))) : ""}</div></div>
        <div class="tile"><div class="kpi-label">Average</div><div class="kpi-value">${num(s.average)}</div><div class="small muted">${s.counted_runs} counted runs</div></div>
        <div class="tile"><div class="kpi-label">Accuracy</div><div class="kpi-value">${acc(s.recent_accuracy)}</div><div class="small muted">Last ${ui.snap.progress_settings.trend_window} runs</div></div>
        <div class="tile"><div class="kpi-label">Improvement</div><div class="kpi-value pos">${pct(s.improvement_pct)}</div><div class="small muted">First run → best</div></div>
      </div>
      ${scoreChart(history, periods)}
    </section>`;
}

// "What the data says" for a KovaaK's scenario.
function scenarioNotesCard({ stats: s, history, cold_starts }) {
  const periods = ui.snap.sens_periods;
  const notes = [];
  for (const c of cold_starts.slice(-3)) {
    const rest = c.vs_rest_of_day_pct !== null ? `, ${pct(c.vs_rest_of_day_pct)} vs. the rest of that day` : "";
    notes.push(["First run on " + fmtShort(day(c.day)), `${num(c.first_score)} vs. previous best ${num(c.previous_best)} (${pct(c.vs_previous_best_pct)}${rest}).`]);
  }
  if (s.plateau) notes.push(["Plateau", `${s.runs_since_pb} runs since your last best and a flat trend (${pct(s.trend_pct)}).`]);
  else if (s.runs_since_pb === 0 && s.counted_runs > 1) notes.push(["New best on your latest run", "Still climbing."]);
  else if (s.runs_since_pb > 0) notes.push(["Since your best", `${s.runs_since_pb} runs. Plateau warnings start after ${ui.snap.progress_settings.plateau_runs}.`]);
  if (s.variation_pct !== null) notes.push(["Consistency", `Scores vary by about ${s.variation_pct.toFixed(0)}% run to run (lower is steadier).`]);
  if (s.trend_pct !== null) notes.push(["Trend", `Last ${ui.snap.progress_settings.trend_window} runs vs. the ones before: ${pct(s.trend_pct)}.`]);
  const lastPeriod = periods[history.at(-1).sens_period];
  return `<section class="card" aria-label="What the data says"><h2>What the data says</h2>
      ${notes.map(([t, b]) => `<div class="note"><div class="note-title">${esc(t)}</div><div class="note-body">${esc(b)}</div></div>`).join("")}
      <div class="small muted">Latest run played at ${esc(sensLabel(lastPeriod.settings))}${lastPeriod.cm_per_360 ? ` · ${lastPeriod.cm_per_360.toFixed(1)} cm/360` : ""}</div>
    </section>`;
}

// A KovaaK's scenario's run table, newest first.
function scenarioRunsCard({ history }) {
  return `<section class="card" aria-label="Runs"><h2>Runs</h2><div class="table-wrap"><table>
      <thead><tr><th>#</th><th>When</th><th class="r">Score</th><th class="r">Accuracy</th><th class="r">Avg TTK</th><th></th></tr></thead>
      <tbody>${history.map((r, i) => ({ r, i })).reverse().slice(0, 60).map(({ r, i }) => `<tr>
        <td class="mono muted">${i + 1}</td><td style="white-space:nowrap">${esc(fmtShort(dt(r.finished_at)))}, ${esc(fmtTime(dt(r.finished_at)))}</td>
        <td class="r num">${num(r.score)}</td><td class="r mono">${acc(r.accuracy)}</td>
        <td class="r mono">${r.avg_ttk_s ? r.avg_ttk_s.toFixed(2) + "s" : "–"}</td>
        <td>${r.restarted ? '<span class="tag grey">restarted</span>' : r.is_pb ? '<span class="tag gold">PB</span>' : ""}</td></tr>`).join("")}</tbody>
    </table></div>${history.length > 60 ? `<div class="small muted">Showing the latest 60 of ${history.length}.</div>` : ""}</section>`;
}

// A score axis from min to max rounded out to a "nice" step (1, 2, 2.5 or 5 × 10^n),
// so tick labels read cleanly; `steps` is how many steps it spans.
function niceScale(min, max) {
  const raw = (max - min || Math.abs(max) || 1) / 4;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw);
  let lo = Math.floor(min / step) * step, hi = Math.ceil(max / step) * step;
  if (hi === max) hi += step;
  if (lo === min && min !== 0) lo -= step;
  if (min >= 0) lo = Math.max(0, lo);
  return { lo, hi, steps: Math.round((hi - lo) / step) };
}

// Score by run, with personal bests, a 3-run average, a line between days and a
// marker where the settings changed. Drawn with the chart helpers in insights.js.
function scoreChart(history, periods) {
  const pts = history.filter((r) => !r.restarted && r.score !== null);
  if (pts.length < 2) return `<div class="note-body">Play this scenario again to see a chart.</div>`;
  const f = { w: 760, h: 260, l: 56, r: 16, t: 16, b: 44 };
  const scores = pts.map((p) => p.score);
  const { lo, hi, steps } = niceScale(Math.min(...scores), Math.max(...scores));
  const last = pts.length - 1;
  const x = (i) => sx(i, 0, last, f);
  const y = (v) => sy(v, lo, hi, f);
  const line = pts.map((p, i) => `${x(i).toFixed(1)},${y(p.score).toFixed(1)}`).join(" ");
  const area = `M${x(0)},${y(lo)} L${line.replaceAll(" ", " L")} L${x(last)},${y(lo)} Z`;
  const avg = pts.length >= 4 ? pts.map((_, i) => {
    const w = scores.slice(Math.max(0, i - 2), i + 1);
    return `${x(i).toFixed(1)},${y(w.reduce((a, b) => a + b, 0) / w.length).toFixed(1)}`;
  }).join(" ") : null;

  const marks = [];
  let lastLabelX = -Infinity;
  pts.forEach((p, i) => {
    const prev = pts[i - 1];
    const between = prev ? (x(i) + x(i - 1)) / 2 : 0;
    if (prev && p.sens_period !== prev.sens_period) {
      const c = periods[p.sens_period];
      marks.push(`<line x1="${between}" y1="${f.t}" x2="${between}" y2="${f.h - f.b}" stroke="var(--blue)" stroke-dasharray="3 4"></line>
        <text x="${between + 4}" y="${f.t + 10}" fill="var(--blue)" font-size="11">${esc(c.cm_per_360 ? c.cm_per_360.toFixed(1) + " cm" : "settings")}</text>`);
    }
    if (!prev || p.day !== prev.day) {
      if (prev) marks.push(`<line x1="${between}" y1="${f.t}" x2="${between}" y2="${f.h - f.b}" stroke="var(--border)"></line>`);
      if (x(i) - lastLabelX > 56) {
        marks.push(`<text x="${x(i)}" y="${f.h - f.b + 30}" fill="var(--muted)" font-size="11" text-anchor="${i === 0 ? "start" : "middle"}">${esc(fmtShort(day(p.day)))}</text>`);
        lastLabelX = x(i);
      }
    }
  });
  const dots = pts.map((p, i) => p.is_pb
    ? `<circle cx="${x(i)}" cy="${y(p.score)}" r="${pts.length > 60 ? 3.5 : 5.5}" fill="var(--gold)" stroke="var(--surface)" stroke-width="2"><title>${esc(num(p.score))} (PB) · ${esc(fmtShort(dt(p.finished_at)))}</title></circle>`
    : pts.length <= 60 ? `<circle cx="${x(i)}" cy="${y(p.score)}" r="3.5" fill="var(--surface)" stroke="var(--muted)" stroke-width="1.5"><title>${esc(num(p.score))} · ${esc(fmtShort(dt(p.finished_at)))}</title></circle>` : "").join("");

  return `<figure style="margin:0">
    <svg viewBox="0 0 ${f.w} ${f.h}" width="100%" role="img" aria-label="Score by run, ${pts.length} runs, best ${esc(num(Math.max(...scores)))}">
      ${yTicks(lo, hi, (v) => esc(num(v)), steps, f)}
      ${marks.join("")}
      <path d="${area}" fill="var(--rust)" fill-opacity="0.1"></path>
      <polyline points="${line}" fill="none" stroke="var(--rust)" stroke-width="2.25" stroke-linejoin="round"></polyline>
      ${avg ? `<polyline points="${avg}" fill="none" stroke="var(--blue)" stroke-width="1.5" stroke-dasharray="6 4"></polyline>` : ""}
      ${dots}
      <text x="${f.l}" y="${f.h - f.b + 14}" fill="var(--faint)" font-size="10">Run 1</text>
      <text x="${f.w - f.r}" y="${f.h - f.b + 14}" fill="var(--faint)" font-size="10" text-anchor="end">Run ${pts.length}</text>
    </svg>
    <figcaption class="legend" style="margin-top:8px">
      <span><i class="swatch" style="background:var(--rust);height:3px;width:14px"></i>Score</span>
      <span><i class="swatch" style="background:var(--gold);border-radius:50%"></i>New personal best</span>
      ${avg ? `<span><i style="width:14px;border-top:2px dashed var(--blue)"></i>3-run average</span>` : ""}
      <span><i style="width:2px;height:12px;border-left:2px dashed var(--blue)"></i>Settings change</span>
    </figcaption></figure>`;
}

// ---------- Progress › Time ----------

function timeTab() {
  const s = ui.snap;
  const t = s.training;
  if (!t.runs.length) return noRunsYet();
  const { active, session, first, today, calendarDays } = totals();
  const goalMet = t.days.filter((d) => d.goal_met).length;
  return `<section class="kpis" aria-label="Totals">
      ${kpi("Active", dur(active), `Inside runs since ${esc(fmtShort(first))}`)}
      ${kpi("Session time", dur(session), `${session ? Math.round((active / session) * 100) : 0}% of it active`)}
      ${kpi("Days trained", `${t.days.length} <small>of ${calendarDays}</small>`, t.streak.trained_today ? "Trained today" : "Today still open")}
      ${kpi("Daily goal", `${goalMet} <small>of ${t.days.length} days</small>`, `${s.time_settings.daily_goal_min} min active`)}
    </section>
    <section class="card" aria-label="Daily time">${dailyChart(7)}</section>
    <div class="row">
      <section class="card grow-3" aria-label="Calendar" style="flex:3 1 420px">${calendar()}</section>
      <section class="card grow-2" aria-label="Sessions" style="flex:2 1 320px">${sessionsList()}</section>
    </div>
    ${fold("time-more", "More about when you train", `<section class="card" aria-label="When you train">${timeline()}</section>
      <div class="row">
        <section class="card grow-2" aria-label="By day of week" style="flex:2 1 320px">${weekdays(first, today)}</section>
        <section class="card grow-3" aria-label="Runs by hour" style="flex:3 1 420px">${hours()}</section>
        <section class="card grow-2" aria-label="Active vs idle" style="flex:2 1 300px">${activeIdle(active, session)}</section>
      </div>`)}`;
}

// Every session, newest first, a page at a time.
function sessionsList() {
  const all = [...ui.snap.training.sessions].reverse();
  const shown = all.slice(0, ui.sessionsShown);
  return `<div class="card-head"><h2>Sessions</h2><span class="small muted">${all.length} in all</span></div>
    <div>${shown.map((x) => `<div class="list-row"><div class="main"><div class="title">${esc(fmtDay(dt(x.start)))}</div>
      <div class="sub">${esc(fmtTime(dt(x.start)))}–${esc(fmtTime(dt(x.end)))} · ${x.runs} run${x.runs === 1 ? "" : "s"}</div></div>
      <div style="text-align:right"><div class="num">${dur(x.active_s)}</div><div class="sub">of ${dur(sessionLength(x))}</div></div></div>`).join("")}</div>
    ${all.length > shown.length ? `<button type="button" class="link-btn" data-action="more-sessions">Show ${Math.min(20, all.length - shown.length)} more</button>` : ""}
    <div class="small muted">A ${ui.snap.time_settings.session_gap_min}-minute break starts a new session.</div>`;
}

function heatClass(minutes) {
  if (minutes <= 0) return "";
  if (minutes < 15) return "l1";
  if (minutes < 45) return "l2";
  if (minutes < 75) return "l3";
  return "l4";
}

function calendar() {
  const t = ui.snap.training;
  const today = day(t.today);
  const month = ui.calMonth ?? new Date(today.getFullYear(), today.getMonth(), 1);
  const byDay = new Map(t.days.map((d) => [d.day, d]));
  const start = addDays(month, -((month.getDay() + 6) % 7));
  const cells = [];
  for (let i = 0; i < 42; i++) {
    const d = addDays(start, i);
    if (i >= 35 && d.getMonth() !== month.getMonth()) break;
    const key = dayKey(d);
    const stat = byDay.get(key);
    const inMonth = d.getMonth() === month.getMonth();
    const mins = stat ? stat.active_s / 60 : 0;
    const cls = ["cal", inMonth ? heatClass(mins) : "out", key === t.today ? "is-today" : ""].join(" ");
    const label = `${fmtDay(d)}: ${stat ? `${dur(stat.active_s)} active, ${stat.runs} runs` : "no training"}`;
    cells.push(`<div class="${cls}" title="${esc(label)}"><span>${d.getDate()}</span>${stat ? `<span class="mins">${dur(stat.active_s)}</span>` : key === t.today ? '<span class="small" style="color:var(--rust-text)">today</span>' : ""}</div>`);
  }
  return `<div class="card-head" style="align-items:center"><h2>${esc(month.toLocaleDateString("en-US", { month: "long", year: "numeric" }))}</h2>
      <div style="display:flex;gap:4px">
        <button type="button" class="icon-btn" data-action="cal-prev" aria-label="Previous month"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" aria-hidden="true"><path d="M15 6l-6 6 6 6"></path></svg></button>
        <button type="button" class="icon-btn" data-action="cal-next" aria-label="Next month"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" aria-hidden="true"><path d="M9 6l6 6-6 6"></path></svg></button>
      </div></div>
    <div class="calendar">${["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map((d) => `<div class="cal-head">${d}</div>`).join("")}${cells.join("")}</div>
    <div class="heat-legend">Less <i style="background:var(--surface-2)"></i><i style="background:var(--heat-1)"></i><i style="background:var(--heat-2)"></i><i style="background:var(--heat-3)"></i><i style="background:var(--heat-4)"></i> More</div>`;
}

function weekdays(first, today) {
  const avg = ui.snap.training.weekday_avg_min;
  const goal = ui.snap.time_settings.daily_goal_min;
  const max = Math.max(goal, ...avg.filter((v) => v !== null)) * 1.1;
  const names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
  const todayIdx = (today.getDay() + 6) % 7;
  const weeks = Math.ceil((Math.round((today - first) / 86400000) + 1) / 7);
  return `<h2>By day of week</h2><div class="small muted">Average active minutes on that weekday. Missed days count as 0.</div>
    <div style="display:flex;flex-direction:column;gap:10px">${avg.map((v, i) => `
      <div class="hbar"><span class="lbl" ${i === todayIdx ? 'style="color:var(--text-strong);font-weight:600"' : ""}>${names[i]}</span>
        <div class="track">${v !== null ? `<div class="fill" style="width:${((v / max) * 100).toFixed(1)}%"></div>` : ""}</div>
        <span class="val ${v === null ? "small muted" : ""}">${v === null ? "no data" : `${Math.round(v)}m`}</span></div>`).join("")}</div>
    ${weeks < 4 ? `<div class="note note-body" style="margin-top:auto">${weeks} ${weeks === 1 ? "week" : "weeks"} of data so far. Weekday averages settle after about four weeks.</div>` : ""}`;
}

function timeline() {
  const t = ui.snap.training;
  const today = day(t.today);
  const days = Array.from({ length: 7 }, (_, i) => addDays(today, i - 6));
  const hourOf = (s, base) => (dt(s) - base) / 3600000;
  const rows = days.map((d) => ({ d, sessions: t.sessions.filter((s) => s.day === dayKey(d)) }));
  let lo = 5, hi = 24;
  for (const r of rows) for (const s of r.sessions) {
    lo = Math.min(lo, Math.floor(hourOf(s.start, r.d)));
    hi = Math.max(hi, Math.ceil(hourOf(s.end, r.d)));
  }
  const pos = (h) => ((h - lo) / (hi - lo)) * 100;
  const ticks = [];
  for (let h = Math.ceil(lo / 3) * 3; h <= hi; h += 3) {
    const hr = ((h % 24) + 24) % 24;
    ticks.push(`<span style="left:${pos(h).toFixed(2)}%">${hr === 0 ? "12a" : hr < 12 ? `${hr}a` : hr === 12 ? "12p" : `${hr - 12}p`}</span>`);
  }
  const body = rows.map(({ d, sessions }) => {
    const isToday = dayKey(d) === t.today;
    const total = sessions.reduce((a, s) => a + s.active_s, 0);
    const blocks = sessions.map((s) => {
      const a = hourOf(s.start, d), b = hourOf(s.end, d);
      return `<div class="tl-block" style="left:${pos(a).toFixed(2)}%;width:${(pos(b) - pos(a)).toFixed(2)}%" title="${esc(`${fmtTime(dt(s.start))}–${fmtTime(dt(s.end))} · ${s.runs} runs · ${dur(s.active_s)} active`)}"></div>`;
    }).join("");
    return `<div class="tl-row"><span class="tl-day" ${isToday ? 'style="color:var(--text-strong);font-weight:600"' : ""}>${esc(d.toLocaleDateString("en-US", { weekday: "short", day: "numeric" }))}</span>
      <div class="tl-track ${sessions.length ? "" : "empty"}">${blocks}</div>
      <span class="tl-total ${sessions.length ? "" : "small muted"}">${sessions.length ? dur(total) : isToday ? "today" : "–"}</span></div>`;
  }).join("");
  return `<div class="card-head"><h2>When you train</h2><div class="small muted">Last 7 days. Each block is a session; a ${ui.snap.time_settings.session_gap_min}+ minute break starts a new one.</div></div>
    <div class="timeline">${body}<div class="tl-row"><span class="tl-day"></span><div class="tl-axis">${ticks.join("")}</div><span class="tl-total"></span></div></div>`;
}

function hours() {
  const h = ui.snap.training.runs_by_hour;
  const max = Math.max(1, ...h);
  const peak = h.indexOf(Math.max(...h));
  const label = (hr) => (hr === 0 ? "12a" : hr < 12 ? `${hr}a` : hr === 12 ? "12p" : `${hr - 12}p`);
  return `<div class="card-head"><h2>Runs by hour of day</h2><div class="small muted">Peak: ${label(peak)} (${h[peak]} runs)</div></div>
    <div class="hours" role="img" aria-label="Runs started in each hour of the day">${h.map((n, i) => `<div style="height:${((n / max) * 100).toFixed(1)}%" title="${label(i)}: ${n} runs"></div>`).join("")}</div>
    <div class="hour-axis">${h.map((_, i) => `<span>${i % 3 === 0 ? label(i) : ""}</span>`).join("")}</div>`;
}

function activeIdle(active, session) {
  const share = session ? active / session : 0;
  const c = 2 * Math.PI * 48;
  const idle = Math.max(0, session - active);
  return `<h2>Active vs. idle</h2>
    <div style="display:flex;align-items:center;gap:20px;flex-wrap:wrap">
      <svg width="120" height="120" viewBox="0 0 120 120" role="img" aria-label="${Math.round(share * 100)}% of session time spent in runs">
        <circle cx="60" cy="60" r="48" fill="none" stroke="#2e333b" stroke-width="14"></circle>
        <circle cx="60" cy="60" r="48" fill="none" stroke="#f0783c" stroke-width="14" stroke-dasharray="${(share * c).toFixed(1)} ${c.toFixed(1)}" transform="rotate(-90 60 60)"></circle>
        <text x="60" y="66" text-anchor="middle" fill="#f3f4f6" font-size="20" font-weight="600" font-family="Cascadia Mono, Consolas, monospace">${Math.round(share * 100)}%</text></svg>
      <div style="flex:1 1 160px;display:flex;flex-direction:column;gap:8px;font-size:13px">
        <div style="display:flex;justify-content:space-between"><span class="muted">In runs</span><span class="mono">${dur(active)}</span></div>
        <div style="display:flex;justify-content:space-between"><span class="muted">Between runs</span><span class="mono">${dur(idle)}</span></div>
        <div style="display:flex;justify-content:space-between"><span class="muted">Idle per session</span><span class="mono">~${dur(idle / ui.snap.training.sessions.length)}</span></div>
      </div></div>
    <div class="note note-body">Idle time is menus, picking scenarios and breaks. Workouts, and playlists in KovaaK's, cut down the menu time.</div>`;
}

// ---------- Progress › Benchmarks ----------

function benchmarksTab() {
  const vt = ui.snap.scenarios.filter((x) => /^VT .* S5\b/.test(x.scenario)).sort((a, b) => a.scenario.localeCompare(b.scenario));
  return `<section class="card"><div class="card-head"><h2>Voltaic Season 5</h2><span class="small muted">${vt.length} scenario${vt.length === 1 ? "" : "s"} played in KovaaK's</span></div>
      <div class="note-body">Your best scores on the Voltaic Season 5 benchmark scenarios, read from your KovaaK's stats. AimAdapt's own scenarios are rated on their own pages and on <a href="#progress/skills">Skills</a> instead.</div>
      ${vt.length ? `<div class="table-wrap"><table><thead><tr><th>Scenario</th><th class="r">Runs</th><th class="r">Best</th><th class="r">Last played</th></tr></thead><tbody>
        ${vt.map((x) => `<tr><td><a href="${scenarioHref(`kovaaks:${x.scenario}`)}">${esc(x.scenario)}</a></td><td class="r mono">${x.runs}</td><td class="r num">${num(x.best_score)}</td>
          <td class="r">${esc(fmtShort(dt(x.last_played)))}</td></tr>`).join("")}</tbody></table></div>`
        : `<div class="note-body">None of them has been played in KovaaK's yet. If you have played them, check where AimAdapt reads your KovaaK's stats in <a href="#settings/kovaaks">Settings › KovaaK's</a>.</div>`}
    </section>`;
}

// ---------- Settings tabs from the old Library page ----------

function kovaaksTab() {
  const s = ui.snap;
  const r = s.last_import;
  const game = s.game;
  const fpsCapped = game && game.avg_fps && game.max_fps && game.avg_fps >= game.max_fps * 0.98;
  const periods = [...s.sens_periods].reverse();
  const kovaaksRuns = s.training.runs.length;
  return `<section class="card">
      <div class="card-head" style="align-items:center"><h2>KovaaK's stats folder</h2>${rescanButton()}</div>
      ${s.stats_dir ? `<div class="path">${esc(s.stats_dir)}</div>` : `<div class="banner"><div><div class="banner-title">KovaaK's wasn't found</div><div class="banner-body">AimAdapt looks in every Steam library for FPSAimTrainer\\stats. Install KovaaK's, then press Rescan folder.</div></div></div>`}
      <div class="small muted">${r ? `${r.found} files in the folder · last scan: ${r.imported} new, ${r.skipped} already imported · ` : ""}${s.import_errors.length} problem file${s.import_errors.length === 1 ? "" : "s"} · ${kovaaksRuns} runs in all. New runs are imported automatically while AimAdapt is open, and AimAdapt never changes your KovaaK's files.</div>
      ${s.import_errors.length ? `<div class="table-wrap"><table><thead><tr><th>File</th><th>Problem</th></tr></thead><tbody>${s.import_errors.map(([f, e]) => `<tr><td>${esc(f)}</td><td class="error">${esc(e)}</td></tr>`).join("")}</tbody></table></div>` : ""}
    </section>
    <section class="card"><h2>Sensitivity history</h2>
      <div class="small muted">From every KovaaK's run. Changes show as markers on score charts.</div>
      ${periods.length ? `<div class="table-wrap"><table><thead><tr><th>Period</th><th>Sens</th><th class="r">DPI</th><th class="r">FOV</th><th class="r">cm/360</th><th class="r">Runs</th></tr></thead><tbody>
        ${periods.map((p, i) => `<tr><td>${esc(fmtShort(dt(p.from)))}${dayKey(dt(p.from)) !== dayKey(dt(p.to)) ? ` – ${esc(fmtShort(dt(p.to)))}` : ""} ${i === 0 ? '<span class="tag">current</span>' : ""}</td>
          <td class="mono">${esc(`${p.settings.horiz_sens ?? "?"} ${p.settings.sens_scale ?? ""}`)}</td><td class="r mono">${p.settings.dpi ?? "–"}</td><td class="r mono">${p.settings.fov ?? "–"}</td>
          <td class="r num">${p.cm_per_360 ? p.cm_per_360.toFixed(1) : "–"}</td><td class="r mono">${p.runs}</td></tr>`).join("")}
      </tbody></table></div>` : `<div class="note-body">No runs to read it from yet.</div>`}
      ${game ? `<div class="kpis" style="grid-template-columns:repeat(auto-fit,minmax(130px,1fr));gap:12px">
        <div class="tile"><div class="small muted">Avg FPS</div><div class="num">${num(game.avg_fps)} <span class="muted">/ cap ${num(game.max_fps)}</span></div></div>
        <div class="tile"><div class="small muted">Resolution</div><div class="num">${esc(game.resolution ?? "–")}</div></div>
        <div class="tile"><div class="small muted">Game version</div><div class="num">${esc((game.game_version ?? "–").split(".").slice(0, 3).join("."))}</div></div></div>` : ""}
      ${fpsCapped ? `<div class="note note-body">FPS sits at the ${num(game.max_fps)} cap. If your monitor refreshes faster, raising the cap in KovaaK's can make tracking feel smoother.</div>` : ""}
    </section>`;
}

function goalsTab() {
  const ts = ui.snap.time_settings;
  return `<section class="card" style="max-width:760px"><h2>Goals and how time is counted</h2>
      <div class="note"><div class="note-title">Daily goal: ${ts.daily_goal_min} minutes active</div><div class="note-body">Today, the streak and the daily-time chart use it. The weekly goal is seven times this.</div></div>
      <div class="note"><div class="note-title">New session after a ${ts.session_gap_min}-minute break</div><div class="note-body">Session time runs from a session's first run to its last, so it includes menus and breaks. Active time is time inside runs.</div></div>
      <div class="note"><div class="note-title">Training day starts at ${ts.day_start_hour}:00</div><div class="note-body">A late-night session counts toward the day it started on.</div></div>
      <div class="note"><div class="note-title">Restarted if under ${Math.round(ts.restart_fraction * 100)}% of the usual length</div><div class="note-body">Compared per scenario (with ${ts.restart_min_runs}+ runs), so 1-second scenarios aren't flagged. Restarted runs never count as bests.</div></div>
    </section>`;
}

function dataTab() {
  const s = ui.snap;
  const d = ui.trainer.data;
  return `<section class="card" style="max-width:760px"><h2>Your data</h2>
      <div class="note-body">Runs, ratings, settings and your own scenarios and workouts stay on this computer. Nothing is sent anywhere.</div>
      <div class="field-label small muted">Database</div><div class="path">${esc(s.db_path)}</div>
      ${d ? `<div class="field-label small muted">Your scenarios</div><div class="path">${esc(d.scenarios_dir)}</div>
        <div class="field-label small muted">Your workouts</div><div class="path">${esc(d.workouts_dir)}</div>
        <div class="small muted">Scenarios and workouts are TOML files, so you can also edit them in a text editor.</div>` : ""}
    </section>`;
}

// ---------- events ----------

document.addEventListener("click", (e) => {
  const action = e.target.closest("[data-action]")?.dataset.action;
  if (action === "rescan") rescan();
  else if (action === "more-sessions") { ui.sessionsShown += 20; render(); }
  else if (action === "all-runs") { ui.allRuns = !ui.allRuns; render(); }
  else if (action === "cal-prev" || action === "cal-next") {
    const today = day(ui.snap.training.today);
    const m = ui.calMonth ?? new Date(today.getFullYear(), today.getMonth(), 1);
    ui.calMonth = new Date(m.getFullYear(), m.getMonth() + (action === "cal-next" ? 1 : -1), 1);
    render();
  }
});

window.addEventListener("hashchange", () => {
  render();
  window.scrollTo(0, 0);
  if (route().view === "help") scrollToHelp();
  if (route().view === "scenarios") restoreListScroll();
});

listen("runs-imported", () => load());
// Wait for every script: the first render needs views from the files loaded after
// this one (trainer.js … today.js, help.js), and data can arrive before they've run.
document.addEventListener("DOMContentLoaded", () => load());
