// RustyAim UI. Plain JS, no build step: it asks the Rust side for a snapshot of
// everything (`snapshot`), renders the current screen into <main>, and reloads
// when the folder watcher reports new runs.

const tauri = window.__TAURI__;
// Outside Tauri (a browser preview), answer from a dump made with
// `rustyaim-app --dump-snapshot dev-snapshot.json` placed next to this file.
const devData = tauri ? null : fetch("dev-snapshot.json").then((r) => r.json());
const invoke = tauri
  ? (cmd, args) => tauri.core.invoke(cmd, args)
  : async (cmd, args) => {
      const d = await devData;
      if (cmd === "snapshot") return d.snapshot;
      if (cmd === "scenario") return d.scenarios[args.name] ?? null;
      if (cmd === "trainer") return d.trainer;
      if (cmd === "check_scenario") return [];
      if (cmd === "rescan") return d.snapshot.last_import;
      throw new Error("Only available in the app");
    };

const ui = {
  snap: null,
  error: null,
  scenarioCache: new Map(),
  progress: { query: "", sort: "played", minRuns: false },
  calMonth: null, // Date for the first of the month shown on Training time
  rescanning: false,
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

async function scenarioDetail(name) {
  if (!ui.scenarioCache.has(name)) ui.scenarioCache.set(name, await invoke("scenario", { name }));
  return ui.scenarioCache.get(name);
}

async function rescan() {
  ui.rescanning = true;
  render();
  try {
    await invoke("rescan");
  } catch (e) {
    ui.error = String(e);
  }
  ui.rescanning = false;
  await load();
}

// ---------- routing ----------

function route() {
  const [view, ...rest] = (location.hash.slice(1) || "dashboard").split("/");
  return { view, arg: rest.length ? decodeURIComponent(rest.join("/")) : null };
}

function render() {
  const { view, arg } = route();
  const navView = view === "edit" || view === "workout" ? "play" : view;
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
  const views = { dashboard, play, edit, workout, insights, progress, training, benchmarks, library, settings };
  main.innerHTML = (views[view] || dashboard)(arg);
  if (view === "progress") afterProgress(arg);
}

function renderStatus() {
  const s = ui.snap;
  const el = document.getElementById("status");
  if (!s) return;
  const watching = !!s.stats_dir;
  const last = s.training.runs.at(-1);
  el.innerHTML = `
    <div class="status-title"><span class="dot ${watching ? "" : "off"}"></span>${watching ? "Watching stats folder" : "Stats folder not found"}</div>
    <div class="small muted">${s.training.runs.length} runs${last ? ` · last run ${esc(fmtShort(dt(last.finished_at)))}, ${esc(fmtTime(dt(last.finished_at)))}` : ""}</div>`;
}

// ---------- shared pieces ----------

const kpi = (label, value, note = "") =>
  `<div class="kpi"><div class="kpi-label">${label}</div><div class="kpi-value">${value}</div>${note ? `<div class="kpi-note">${note}</div>` : ""}</div>`;

function head(eyebrow, title, actions = "") {
  return `<header class="page-head"><div><div class="eyebrow">${eyebrow}</div><h1>${title}</h1></div>${actions}</header>`;
}

const rescanButton = () =>
  `<button class="btn" type="button" data-action="rescan" ${ui.rescanning ? "disabled" : ""}>
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.3 5.7"></path><path d="M20 4v7h-7"></path></svg>
    ${ui.rescanning ? "Scanning…" : "Rescan folder"}</button>`;

function emptyIfNoRuns() {
  if (ui.snap.training.runs.length) return null;
  return `<div class="page">${head("Welcome", "No runs yet")}
    <div class="card"><div class="note-body">${ui.snap.stats_dir ? "Play a scenario in KovaaK's and it will show up here." : "The KovaaK's stats folder wasn't found. See Library &amp; import."}</div></div></div>`;
}

function totals() {
  const t = ui.snap.training;
  const active = t.sessions.reduce((a, s) => a + s.active_s, 0);
  const session = t.sessions.reduce((a, s) => a + sessionLength(s), 0);
  const first = day(t.days[0].day);
  const today = day(t.today);
  const calendarDays = Math.round((today - first) / 86400000) + 1;
  return { active, session, first, today, calendarDays };
}

// Best 4-hour window of the day by runs started.
function peakWindow() {
  const h = ui.snap.training.runs_by_hour;
  const total = h.reduce((a, b) => a + b, 0);
  let best = 0, start = 0;
  for (let i = 0; i <= 20; i++) {
    const sum = h[i] + h[i + 1] + h[i + 2] + h[i + 3];
    if (sum > best) [best, start] = [sum, i];
  }
  const label = (hr) => new Date(2000, 0, 1, hr).toLocaleTimeString("en-US", { hour: "numeric" });
  return total ? { share: Math.round((best / total) * 100), from: label(start), to: label(start + 4) } : null;
}

// ---------- Dashboard ----------

function dashboard() {
  const empty = emptyIfNoRuns();
  if (empty) return empty;
  const s = ui.snap;
  const t = s.training;
  const { active, session, first, today } = totals();
  const goalMin = s.time_settings.daily_goal_min;
  const scenarios = s.scenarios;

  const monday = addDays(today, -((today.getDay() + 6) % 7));
  const week = t.weeks.find((w) => w.week_start === dayKey(monday));
  const weekActive = week ? week.active_s : 0;
  const weekGoal = goalMin * 7 * 60;

  const todayStat = t.days.find((d) => d.day === t.today);
  const peak = peakWindow();
  const peakText = peak ? `${peak.share}% of your runs start between ${peak.from} and ${peak.to}.` : "";
  const banner = t.streak.trained_today
    ? `<section class="banner"><div style="flex:1 1 300px"><div class="banner-title">Trained today: ${dur(todayStat.active_s)} active${todayStat.goal_met ? ", daily goal met" : ` of your ${goalMin}-minute goal`}.</div><div class="banner-body">${peakText}</div></div></section>`
    : t.streak.current > 0
      ? `<section class="banner"><div style="flex:1 1 300px"><div class="banner-title">No training yet today. Your ${t.streak.current}-day streak ends when the day does.</div><div class="banner-body">${peakText}</div></div></section>`
      : "";

  return `<div class="page">
    ${head(esc(today.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" })), "Dashboard", rescanButton())}
    ${banner}
    <section class="kpis" aria-label="Totals">
      ${kpi("Active training", dur(active), `Inside runs since ${esc(fmtShort(first))}`)}
      ${kpi("Session time", dur(session), `Incl. menus and breaks · ${session ? Math.round((active / session) * 100) : 0}% active`)}
      ${kpi("Runs", t.runs.length, `Across ${scenarios.length} scenarios`)}
      ${kpi("Streak", `<span style="color:var(--rust)">${t.streak.current} ${t.streak.current === 1 ? "day" : "days"}</span>`, `Best ${t.streak.best} · ${t.streak.trained_today ? "trained today" : "today pending"}`)}
      <div class="kpi"><div class="kpi-label">This week</div><div class="kpi-value">${dur(weekActive)}</div>
        <div class="bar-track"><div class="bar-fill" style="width:${Math.min(100, (weekActive / weekGoal) * 100).toFixed(0)}%"></div></div>
        <div class="kpi-note">${Math.round((weekActive / weekGoal) * 100)}% of ${dur(weekGoal)} weekly goal</div></div>
    </section>
    <div class="row">
      <section class="card grow-3" aria-label="Daily time">${dailyChart(7)}</section>
      <section class="card grow-2" aria-label="Most played">
        <div class="card-head"><h2>Most played</h2><a href="#progress" class="small">All scenarios</a></div>
        <div>${[...scenarios].sort((a, b) => b.runs - a.runs).slice(0, 6).map((x) => `
          <a class="list-row" href="#progress/${encodeURIComponent(x.scenario)}" style="color:inherit">
            <div class="main"><div class="title">${esc(x.scenario)}</div><div class="sub">${x.runs} runs · best ${num(x.best_score)}</div></div>
            <div class="num pos">${pct(x.improvement_pct)}</div></a>`).join("")}</div>
        <div class="small muted">Skill split (clicking / tracking / switching) arrives with scenario tags.</div>
      </section>
    </div>
    <div class="grid">
      <section class="card" aria-label="Recent sessions">
        <div class="card-head"><h2>Recent sessions</h2><a href="#training" class="small">All ${t.sessions.length}</a></div>
        <div>${[...t.sessions].reverse().slice(0, 5).map((x) => `
          <div class="list-row"><div class="main"><div class="title">${esc(fmtDay(dt(x.start)))}</div>
            <div class="sub">${esc(fmtTime(dt(x.start)))}–${esc(fmtTime(dt(x.end)))} · ${x.runs} runs</div></div>
            <div style="text-align:right"><div class="num">${dur(x.active_s)}</div><div class="sub">of ${dur(sessionLength(x))}</div></div></div>`).join("")}</div>
      </section>
      <section class="card" aria-label="Most improved">
        <div class="card-head"><h2>Most improved</h2><a href="#progress" class="small">All scenarios</a></div>
        <div>${scenarios.filter((x) => x.counted_runs >= 3 && x.improvement_pct !== null)
          .sort((a, b) => b.improvement_pct - a.improvement_pct).slice(0, 5).map((x) => `
          <a class="list-row" href="#progress/${encodeURIComponent(x.scenario)}" style="color:inherit">
            <div class="main"><div class="title">${esc(x.scenario)}</div><div class="sub mono">${num(x.first_score)} → ${num(x.best_score)}</div></div>
            <div class="num pos">${pct(x.improvement_pct)}</div></a>`).join("") || `<div class="note-body">Play a scenario 3 or more times to see improvement.</div>`}</div>
      </section>
      <section class="card" aria-label="Coach notes"><h2>Coach notes</h2>${coachNotes().map((n) => `
        <div class="note"><div class="note-title">${esc(n.title)}</div><div class="note-body">${esc(n.body)}</div></div>`).join("")}</section>
    </div>
  </div>`;
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

function coachNotes() {
  const s = ui.snap;
  const notes = [];
  const runs = s.training.runs.length;
  const sc = s.scenarios;
  const perScenario = runs / Math.max(1, sc.length);
  if (perScenario < 3) {
    const played = sc.filter((x) => x.counted_runs >= 5);
    const later = played.filter((x) => (x.improvement_pct ?? 0) > 0).length;
    notes.push({
      title: "Repeat before you switch",
      body: `${sc.length} scenarios in ${runs} runs is ${perScenario.toFixed(1)} runs each.` +
        (played.length ? ` In your ${played.length} scenarios with 5+ runs, the best came after the first attempt in ${later}.` : "") +
        " Try 3 to 5 runs per scenario.",
    });
  }
  const warm = s.cold_starts.map((c) => c.vs_rest_of_day_pct).filter((v) => v !== null);
  const warmMedian = median(warm);
  if (warm.length >= 3 && warmMedian < -5) {
    notes.push({
      title: "Warm up first",
      body: `Your first run of the day on a scenario is typically ${Math.abs(warmMedian).toFixed(0)}% below your later runs that day (${warm.length} days measured). Start with an easy scenario.`,
    });
  }
  const plateaus = sc.filter((x) => x.plateau);
  if (plateaus.length) {
    notes.push({
      title: `Plateau on ${plateaus.length === 1 ? plateaus[0].scenario : `${plateaus.length} scenarios`}`,
      body: `${s.progress_settings.plateau_runs}+ runs without a new best and a flat trend. Try an easier or harder variant for a while.`,
    });
  }
  const periods = s.sens_periods;
  if (periods.length > 1) {
    const [prev, cur] = periods.slice(-2);
    const cm = (p) => (p.cm_per_360 ? `${p.cm_per_360.toFixed(1)} cm/360` : sensLabel(p.settings));
    notes.push({
      title: `Settings changed on ${fmtShort(dt(cur.from))}`,
      body: `${cm(prev)} (${sensLabel(prev.settings)}) → ${cm(cur)} (${sensLabel(cur.settings)}). Progress charts mark changes.`,
    });
  }
  if (!notes.length) notes.push({ title: "Keep going", body: "Notes appear here as patterns show up in your runs." });
  return notes.slice(0, 3);
}

// ---------- Progress ----------

function sortedScenarios() {
  const { query, sort, minRuns } = ui.progress;
  const q = query.trim().toLowerCase();
  const list = ui.snap.scenarios.filter((x) => (!q || x.scenario.toLowerCase().includes(q)) && (!minRuns || x.counted_runs >= 3));
  const by = {
    played: (a, b) => b.runs - a.runs || (b.last_played > a.last_played ? 1 : -1),
    improved: (a, b) => (b.improvement_pct ?? -Infinity) - (a.improvement_pct ?? -Infinity),
    recent: (a, b) => (b.last_played > a.last_played ? 1 : -1),
    name: (a, b) => a.scenario.localeCompare(b.scenario),
  };
  return list.sort(by[sort]);
}

function progress(selected) {
  const empty = emptyIfNoRuns();
  if (empty) return empty;
  const list = sortedScenarios();
  const current = selected ?? list[0]?.scenario ?? null;
  const p = ui.progress;
  const items = list.map((x) => `
    <button type="button" class="scenario-item ${x.scenario === current ? "on" : ""}" data-scenario="${esc(x.scenario)}" ${x.scenario === current ? 'aria-pressed="true"' : ""}>
      <span style="flex:1;min-width:0"><span class="name">${esc(x.scenario)}</span>
      <span class="sub">${x.runs} runs · best ${num(x.best_score)}${x.plateau ? " · plateau" : ""}</span></span>
      <span class="num pos">${pct(x.improvement_pct)}</span></button>`).join("");
  return `<div class="page">
    ${head("Scenario history", "Progress", `<div style="display:flex;gap:8px;align-items:center"><label for="sort" class="small muted">Sort</label>
      <select id="sort" class="select">${["played:Most played", "improved:Most improved", "recent:Recently played", "name:Name"].map((o) => {
        const [v, l] = o.split(":");
        return `<option value="${v}" ${p.sort === v ? "selected" : ""}>${l}</option>`;
      }).join("")}</select></div>`)}
    <div class="row" style="align-items:flex-start">
      <section class="card grow-1" aria-label="Scenarios" style="padding:16px;gap:12px">
        <label for="q" class="sr-only">Search scenarios</label>
        <div class="field"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9aa1ac" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"></circle><path d="M20 20l-4-4"></path></svg>
          <input id="q" type="search" placeholder="Search ${ui.snap.scenarios.length} scenarios" value="${esc(p.query)}"></div>
        <div class="chips"><button type="button" class="chip ${p.minRuns ? "on" : ""}" data-action="min-runs" aria-pressed="${p.minRuns}">3+ runs</button></div>
        <div class="scenario-list">${items || `<div class="note-body">No scenarios match.</div>`}</div>
      </section>
      <div class="grow-3" style="flex:2 1 560px;display:flex;flex-direction:column;gap:16px" id="detail">
        ${current ? `<div class="card"><div class="muted">Loading ${esc(current)}…</div></div>` : ""}
      </div>
    </div>
  </div>`;
}

async function afterProgress(selected) {
  const current = selected ?? sortedScenarios()[0]?.scenario;
  if (!current) return;
  const detail = await scenarioDetail(current);
  const el = document.getElementById("detail");
  if (!el || route().view !== "progress") return;
  el.innerHTML = detail ? scenarioView(detail) : `<div class="card">No runs for ${esc(current)}.</div>`;
}

function scenarioView({ stats: s, history, cold_starts }) {
  const periods = ui.snap.sens_periods;
  const counted = history.filter((r) => !r.restarted && r.score !== null);
  const sessionsCount = new Set(counted.map((r) => r.day)).size;
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

  return `<section class="card" aria-label="Selected scenario" style="gap:18px">
      <div><h2 style="font-size:22px">${esc(s.scenario)}</h2>
        <div class="small muted" style="margin-top:6px">${s.runs} runs${s.runs !== s.counted_runs ? ` (${s.runs - s.counted_runs} restarted)` : ""} · ${sessionsCount} ${sessionsCount === 1 ? "day" : "days"} · first played ${esc(fmtShort(dt(s.first_played)))}</div></div>
      <div class="kpis" style="grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:12px">
        <div class="tile"><div class="kpi-label">Personal best</div><div class="kpi-value" style="color:var(--gold)">${num(s.best_score)}</div><div class="small muted">${s.best_at ? esc(fmtShort(dt(s.best_at)) + ", " + fmtTime(dt(s.best_at))) : ""}</div></div>
        <div class="tile"><div class="kpi-label">Average</div><div class="kpi-value">${num(s.average)}</div><div class="small muted">${s.counted_runs} counted runs</div></div>
        <div class="tile"><div class="kpi-label">Accuracy</div><div class="kpi-value">${acc(s.recent_accuracy)}</div><div class="small muted">Last ${ui.snap.progress_settings.trend_window} runs</div></div>
        <div class="tile"><div class="kpi-label">Improvement</div><div class="kpi-value pos">${pct(s.improvement_pct)}</div><div class="small muted">First run → best</div></div>
      </div>
      ${scoreChart(history, periods)}
    </section>
    <div class="row" style="align-items:flex-start">
      <section class="card grow-3" aria-label="Runs" style="flex:3 1 360px"><h3>Runs</h3><div class="table-wrap"><table>
        <thead><tr><th>#</th><th>When</th><th class="r">Score</th><th class="r">Accuracy</th><th class="r">Avg TTK</th><th></th></tr></thead>
        <tbody>${history.map((r, i) => ({ r, i })).reverse().slice(0, 60).map(({ r, i }) => `<tr>
          <td class="mono muted">${i + 1}</td><td style="white-space:nowrap">${esc(fmtShort(dt(r.finished_at)))}, ${esc(fmtTime(dt(r.finished_at)))}</td>
          <td class="r num">${num(r.score)}</td><td class="r mono">${acc(r.accuracy)}</td>
          <td class="r mono">${r.avg_ttk_s ? r.avg_ttk_s.toFixed(2) + "s" : "–"}</td>
          <td>${r.restarted ? '<span class="tag grey">restarted</span>' : r.is_pb ? '<span class="tag gold">PB</span>' : ""}</td></tr>`).join("")}</tbody>
      </table></div>${history.length > 60 ? `<div class="small muted">Showing the latest 60 of ${history.length}.</div>` : ""}</section>
      <section class="card grow-2" aria-label="Insights" style="flex:2 1 280px"><h3>What the data says</h3>
        ${notes.map(([t, b]) => `<div class="note"><div class="note-title">${esc(t)}</div><div class="note-body">${esc(b)}</div></div>`).join("")}
        <div class="small muted">Latest run played at ${esc(sensLabel(lastPeriod.settings))}${lastPeriod.cm_per_360 ? ` · ${lastPeriod.cm_per_360.toFixed(1)} cm/360` : ""}</div>
      </section>
    </div>`;
}

function scoreChart(history, periods) {
  const pts = history.filter((r) => !r.restarted && r.score !== null);
  if (pts.length < 2) return `<div class="note-body">Play this scenario again to see a chart.</div>`;
  const W = 760, H = 260, L = 56, R = 16, T = 16, B = 44;
  const scores = pts.map((p) => p.score);
  // Round the axis to a "nice" step (1, 2, 2.5 or 5 × 10^n) so tick labels read cleanly.
  const minS = Math.min(...scores), maxS = Math.max(...scores);
  const raw = (maxS - minS || Math.abs(maxS) || 1) / 4;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw);
  let lo = Math.floor(minS / step) * step, hi = Math.ceil(maxS / step) * step;
  if (hi === maxS) hi += step;
  if (lo === minS && minS !== 0) lo -= step;
  if (minS >= 0) lo = Math.max(0, lo);
  const x = (i) => L + (pts.length === 1 ? 0 : (i / (pts.length - 1)) * (W - L - R));
  const y = (v) => T + (1 - (v - lo) / (hi - lo)) * (H - T - B);
  const ticks = [];
  for (let v = lo; v <= hi + step / 2; v += step) ticks.push(v);
  const line = pts.map((p, i) => `${x(i).toFixed(1)},${y(p.score).toFixed(1)}`).join(" ");
  const area = `M${x(0)},${y(lo)} L${line.replaceAll(" ", " L")} L${x(pts.length - 1)},${y(lo)} Z`;
  const avg = pts.length >= 4 ? pts.map((_, i) => {
    const w = scores.slice(Math.max(0, i - 2), i + 1);
    return `${x(i).toFixed(1)},${y(w.reduce((a, b) => a + b, 0) / w.length).toFixed(1)}`;
  }).join(" ") : null;

  const marks = [];
  let lastLabelX = -Infinity;
  pts.forEach((p, i) => {
    const prev = pts[i - 1];
    if (prev && p.sens_period !== prev.sens_period) {
      const mx = (x(i) + x(i - 1)) / 2;
      const c = periods[p.sens_period];
      marks.push(`<line x1="${mx}" y1="${T}" x2="${mx}" y2="${H - B}" stroke="#5aa9ff" stroke-dasharray="3 4"></line>
        <text x="${mx + 4}" y="${T + 10}" fill="#5aa9ff" font-size="11">${esc(c.cm_per_360 ? c.cm_per_360.toFixed(1) + " cm" : "settings")}</text>`);
    }
    if (!prev || p.day !== prev.day) {
      if (prev) marks.push(`<line x1="${(x(i) + x(i - 1)) / 2}" y1="${T}" x2="${(x(i) + x(i - 1)) / 2}" y2="${H - B}" stroke="#262a31"></line>`);
      if (x(i) - lastLabelX > 56) {
        marks.push(`<text x="${x(i)}" y="${H - B + 30}" fill="#aeb4be" font-size="11" text-anchor="${i === 0 ? "start" : "middle"}">${esc(fmtShort(day(p.day)))}</text>`);
        lastLabelX = x(i);
      }
    }
  });
  const dots = pts.map((p, i) => p.is_pb
    ? `<circle cx="${x(i)}" cy="${y(p.score)}" r="${pts.length > 60 ? 3.5 : 5.5}" fill="#f2c14e" stroke="#16181c" stroke-width="2"><title>${esc(num(p.score))} (PB) · ${esc(fmtShort(dt(p.finished_at)))}</title></circle>`
    : pts.length <= 60 ? `<circle cx="${x(i)}" cy="${y(p.score)}" r="3.5" fill="#16181c" stroke="#aeb4be" stroke-width="1.5"><title>${esc(num(p.score))} · ${esc(fmtShort(dt(p.finished_at)))}</title></circle>` : "").join("");

  return `<figure style="margin:0">
    <svg viewBox="0 0 ${W} ${H}" width="100%" role="img" aria-label="Score by run, ${pts.length} runs, best ${esc(num(Math.max(...scores)))}">
      ${ticks.map((v) => `<line x1="${L}" y1="${y(v)}" x2="${W - R}" y2="${y(v)}" stroke="#262a31"></line>
        <text x="${L - 8}" y="${y(v) + 4}" fill="#9aa1ac" font-size="11" text-anchor="end" font-family="Cascadia Mono, Consolas, monospace">${esc(num(v))}</text>`).join("")}
      ${marks.join("")}
      <path d="${area}" fill="#f0783c" fill-opacity="0.1"></path>
      <polyline points="${line}" fill="none" stroke="#f0783c" stroke-width="2.25" stroke-linejoin="round"></polyline>
      ${avg ? `<polyline points="${avg}" fill="none" stroke="#5aa9ff" stroke-width="1.5" stroke-dasharray="6 4"></polyline>` : ""}
      ${dots}
      <text x="${L}" y="${H - B + 14}" fill="#7c8490" font-size="10">Run 1</text>
      <text x="${W - R}" y="${H - B + 14}" fill="#7c8490" font-size="10" text-anchor="end">Run ${pts.length}</text>
    </svg>
    <figcaption class="legend" style="margin-top:8px">
      <span><i class="swatch" style="background:#f0783c;height:3px;width:14px"></i>Score</span>
      <span><i class="swatch" style="background:#f2c14e;border-radius:50%"></i>New personal best</span>
      ${avg ? `<span><i style="width:14px;border-top:2px dashed #5aa9ff"></i>3-run average</span>` : ""}
      <span><i style="width:2px;height:12px;border-left:2px dashed #5aa9ff"></i>Settings change</span>
    </figcaption></figure>`;
}

// ---------- Training time ----------

function training() {
  const empty = emptyIfNoRuns();
  if (empty) return empty;
  const s = ui.snap;
  const t = s.training;
  const { active, session, first, today, calendarDays } = totals();
  const goalMet = t.days.filter((d) => d.goal_met).length;
  return `<div class="page">
    ${head(`Since ${esc(fmtDay(first))} · ${calendarDays} ${calendarDays === 1 ? "day" : "days"}`, "Training time")}
    <section class="kpis" aria-label="Totals">
      ${kpi("Active", dur(active), "Inside runs")}
      ${kpi("Session", dur(session), "First run to last, per session")}
      ${kpi("Days trained", `${t.days.length} <small>of ${calendarDays}</small>`, t.streak.trained_today ? "Trained today" : "Today still open")}
      ${kpi("Per training day", dur(active / t.days.length), "Active, average")}
      ${kpi("Per session", dur(active / t.sessions.length), `${t.sessions.length} sessions · ${dur(session / t.sessions.length)} with breaks`)}
      ${kpi("Daily goal", `${goalMet} <small>of ${t.days.length} days</small>`, `${s.time_settings.daily_goal_min} min active`)}
    </section>
    <div class="row">
      <section class="card grow-3" aria-label="Calendar" style="flex:3 1 420px">${calendar()}</section>
      <section class="card grow-2" aria-label="By day of week" style="flex:2 1 320px">${weekdays(first, today)}</section>
    </div>
    <section class="card" aria-label="When you train">${timeline()}</section>
    <div class="row">
      <section class="card grow-3" aria-label="Runs by hour" style="flex:3 1 420px">${hours()}</section>
      <section class="card grow-2" aria-label="Active vs idle" style="flex:2 1 320px">${activeIdle(active, session)}</section>
    </div>
  </div>`;
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
    <div class="note note-body">Idle time is menus, picking scenarios and breaks. A saved playlist cuts down the menu time.</div>`;
}

// ---------- Benchmarks ----------

function benchmarks() {
  const vt = ui.snap.scenarios.filter((x) => /^VT .* S5\b/.test(x.scenario)).sort((a, b) => a.scenario.localeCompare(b.scenario));
  return `<div class="page">
    ${head("Standard scenario sets with rank targets", "Benchmarks")}
    <section class="banner info"><div style="flex:1 1 320px"><div class="banner-title">Rank thresholds aren't loaded yet</div>
      <div class="banner-body">Benchmark packs and ranks come with scenario tags (backlog Epic 4). Until then, here are your bests on Voltaic Season 5 scenarios from your stats files.</div></div></section>
    <section class="card"><div class="card-head"><h2>Voltaic S5 scenarios played</h2><span class="small muted">${vt.length} scenarios</span></div>
      ${vt.length ? `<div class="table-wrap"><table><thead><tr><th>Scenario</th><th class="r">Runs</th><th class="r">Best</th><th class="r">Last played</th><th>Rank</th></tr></thead><tbody>
        ${vt.map((x) => `<tr><td><a href="#progress/${encodeURIComponent(x.scenario)}">${esc(x.scenario)}</a></td><td class="r mono">${x.runs}</td><td class="r num">${num(x.best_score)}</td>
          <td class="r">${esc(fmtShort(dt(x.last_played)))}</td><td class="muted">–</td></tr>`).join("")}</tbody></table></div>`
        : `<div class="note-body">No Voltaic S5 scenarios played yet.</div>`}
    </section>
  </div>`;
}

// ---------- Library & import ----------

function library() {
  const s = ui.snap;
  const t = s.training;
  const r = s.last_import;
  const runs = t.runs;
  const ts = s.time_settings;
  const game = s.game;
  const fpsCapped = game && game.avg_fps && game.max_fps && game.avg_fps >= game.max_fps * 0.98;
  const periods = [...s.sens_periods].reverse();
  return `<div class="page">
    ${head("Data source, rules and settings history", "Library &amp; import")}
    <section class="card">
      <div class="card-head" style="align-items:center"><h2>KovaaK's stats folder</h2>${rescanButton()}</div>
      ${s.stats_dir ? `<div class="path">${esc(s.stats_dir)}</div>` : `<div class="banner"><div><div class="banner-title">Stats folder not found</div><div class="banner-body">RustyAim looks in every Steam library for FPSAimTrainer\\stats. Choosing a folder by hand is on the backlog.</div></div></div>`}
      <div class="kpis" style="grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px">
        <div class="tile"><div class="small muted">Runs in database</div><div class="kpi-value">${runs.length}</div></div>
        <div class="tile"><div class="small muted">Scenarios</div><div class="kpi-value">${s.scenarios.length}</div></div>
        <div class="tile"><div class="small muted">Files in folder</div><div class="kpi-value">${r ? r.found : "–"}</div></div>
        <div class="tile"><div class="small muted">Import errors</div><div class="kpi-value">${s.import_errors.length}</div></div>
        <div class="tile"><div class="small muted">Date range</div><div class="kpi-value" style="font-size:16px">${runs.length ? `${esc(fmtShort(dt(runs[0].started_at)))} – ${esc(fmtShort(dt(runs.at(-1).finished_at)))}` : "–"}</div></div>
      </div>
      <div class="small muted">${r ? `Last scan: ${r.imported} new, ${r.skipped} already imported.` : ""} New runs are imported automatically while RustyAim is open. Database: <span class="mono">${esc(s.db_path)}</span></div>
      ${s.import_errors.length ? `<div class="table-wrap"><table><thead><tr><th>File</th><th>Problem</th></tr></thead><tbody>${s.import_errors.map(([f, e]) => `<tr><td>${esc(f)}</td><td class="error">${esc(e)}</td></tr>`).join("")}</tbody></table></div>` : ""}
    </section>
    <div class="row">
      <section class="card grow-1" style="flex:1 1 360px"><h2>How time is counted</h2>
        <div class="note"><div class="note-title">New session after a ${ts.session_gap_min}-minute break</div><div class="note-body">Session time runs from a session's first run to its last, so it includes menus and breaks.</div></div>
        <div class="note"><div class="note-title">Training day starts at ${ts.day_start_hour}:00</div><div class="note-body">A late-night session counts toward the day it started on.</div></div>
        <div class="note"><div class="note-title">Restarted if under ${Math.round(ts.restart_fraction * 100)}% of the usual length</div><div class="note-body">Compared per scenario (with ${ts.restart_min_runs}+ runs), so 1-second scenarios aren't flagged. Restarted runs never count as bests.</div></div>
        <div class="note"><div class="note-title">Daily goal: ${ts.daily_goal_min} minutes active</div><div class="note-body">Editing these is coming with the settings file.</div></div>
      </section>
      <section class="card grow-1" style="flex:1 1 420px"><h2>Sensitivity history</h2>
        <div class="small muted">Read from every run file. Changes show as markers on progress charts.</div>
        <div class="table-wrap"><table><thead><tr><th>Period</th><th>Sens</th><th class="r">DPI</th><th class="r">FOV</th><th class="r">cm/360</th><th class="r">Runs</th></tr></thead><tbody>
          ${periods.map((p, i) => `<tr><td>${esc(fmtShort(dt(p.from)))}${dayKey(dt(p.from)) !== dayKey(dt(p.to)) ? ` – ${esc(fmtShort(dt(p.to)))}` : ""} ${i === 0 ? '<span class="tag">current</span>' : ""}</td>
            <td class="mono">${esc(`${p.settings.horiz_sens ?? "?"} ${p.settings.sens_scale ?? ""}`)}</td><td class="r mono">${p.settings.dpi ?? "–"}</td><td class="r mono">${p.settings.fov ?? "–"}</td>
            <td class="r num">${p.cm_per_360 ? p.cm_per_360.toFixed(1) : "–"}</td><td class="r mono">${p.runs}</td></tr>`).join("")}
        </tbody></table></div>
        ${game ? `<div class="kpis" style="grid-template-columns:repeat(auto-fit,minmax(130px,1fr));gap:12px">
          <div class="tile"><div class="small muted">Avg FPS</div><div class="num">${num(game.avg_fps)} <span class="muted">/ cap ${num(game.max_fps)}</span></div></div>
          <div class="tile"><div class="small muted">Resolution</div><div class="num">${esc(game.resolution ?? "–")}</div></div>
          <div class="tile"><div class="small muted">Game version</div><div class="num">${esc((game.game_version ?? "–").split(".").slice(0, 3).join("."))}</div></div></div>` : ""}
        ${fpsCapped ? `<div class="note note-body">FPS sits at the ${num(game.max_fps)} cap. If your monitor refreshes faster, raising the cap in KovaaK's can make tracking feel smoother.</div>` : ""}
      </section>
    </div>
  </div>`;
}

// ---------- events ----------

document.addEventListener("click", (e) => {
  const action = e.target.closest("[data-action]")?.dataset.action;
  const item = e.target.closest("[data-scenario]");
  if (action === "rescan") rescan();
  else if (action === "min-runs") { ui.progress.minRuns = !ui.progress.minRuns; render(); }
  else if (action === "cal-prev" || action === "cal-next") {
    const today = day(ui.snap.training.today);
    const m = ui.calMonth ?? new Date(today.getFullYear(), today.getMonth(), 1);
    ui.calMonth = new Date(m.getFullYear(), m.getMonth() + (action === "cal-next" ? 1 : -1), 1);
    render();
  } else if (item) location.hash = `#progress/${encodeURIComponent(item.dataset.scenario)}`;
});

document.addEventListener("input", (e) => {
  if (e.target.id === "q") {
    ui.progress.query = e.target.value;
    const pos = e.target.selectionStart;
    render();
    const q = document.getElementById("q");
    q.focus();
    q.setSelectionRange(pos, pos);
  }
});

document.addEventListener("change", (e) => {
  if (e.target.id === "sort") { ui.progress.sort = e.target.value; render(); }
});

window.addEventListener("hashchange", () => {
  render();
  window.scrollTo(0, 0);
});

if (tauri) tauri.event.listen("runs-imported", () => load());
load();
