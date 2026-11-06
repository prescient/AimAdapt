// Scenarios: one list of everything you can play or have played (the trainer's
// scenarios, your workouts and your KovaaK's scenarios), newest first, beside the
// selected one. The selection is one scrolling page: Play at the top, then key
// figures, the rating chart, what to work on, Insights and the runs.
//
// Loaded after app.js, trainer.js, insights.js and program.js; it draws with their
// helpers and data (ui.trainer.data, ui.insights, ui.snap).

ui.list = { query: "", skill: "", tag: "", source: "", scroll: 0 };

// Built-in workouts for the program's days live on Today and Progress, not here.
const isProgramDay = (e) => e.builtin && /^builtin:core-/.test(e.id);

// Everything in the list, newest first; never-played items last, by name.
function scenarioItems() {
  const d = ui.trainer.data;
  const overview = new Map((ui.insights.overview ?? []).map((s) => [s.scenario, s]));
  const later = (a, b) => (!a ? b : !b ? a : a > b ? a : b);
  const items = [];
  const names = new Set();
  for (const e of d.scenarios) {
    if (!e.scenario) continue;
    const s = e.scenario;
    names.add(s.name);
    const o = overview.get(s.name);
    items.push({
      key: e.id,
      kind: "trainer",
      name: s.name,
      skill: SKILL_LABELS[s.skill] ?? "",
      tags: s.tags ?? [],
      last: later(e.last_played, o?.last_played ?? null),
      figure: e.rating !== null && e.rating !== undefined ? fmtKnob(e.knob, e.rating) : e.runs ? num(e.best) : "",
      mine: !e.builtin,
      entry: e,
    });
  }
  for (const e of d.workouts) {
    if (!e.workout || isProgramDay(e)) continue;
    items.push({
      key: `workout:${e.id}`,
      kind: "workout",
      name: e.workout.name,
      skill: "Workout",
      tags: [],
      last: e.last_played,
      figure: `${e.workout.items.reduce((a, i) => a + i.runs, 0)} runs`,
      mine: !e.builtin,
      entry: e,
    });
  }
  for (const s of ui.snap.scenarios) {
    if (names.has(s.scenario)) continue;
    items.push({ key: `kovaaks:${s.scenario}`, kind: "kovaaks", name: s.scenario, skill: "", tags: [], last: s.last_played, figure: num(s.best_score), entry: s });
  }
  return items.sort((a, b) => (a.last && b.last ? (a.last < b.last ? 1 : -1) : a.last ? -1 : b.last ? 1 : a.name.localeCompare(b.name)));
}

function dayLabel(last) {
  if (!last) return "Not played yet";
  const d = dt(last);
  const today = day(ui.snap.training.today);
  const days = Math.round((new Date(d.getFullYear(), d.getMonth(), d.getDate()) - today) / 86400000);
  if (days >= 0) return "Today";
  if (days === -1) return "Yesterday";
  if (days > -7) return d.toLocaleDateString("en-US", { weekday: "long" });
  return fmtShort(d);
}

function filteredItems(items) {
  const f = ui.list;
  const q = f.query.trim().toLowerCase();
  return items.filter((x) =>
    (!q || x.name.toLowerCase().includes(q)) &&
    (!f.skill || x.skill === f.skill) &&
    (!f.tag || x.tags.includes(f.tag)) &&
    (!f.source || (f.source === "kovaaks" ? x.kind === "kovaaks" : x.kind !== "kovaaks")));
}

// The item a route argument names. "name:<scenario>" (links from older pages and
// from Today) finds it by name.
function findItem(items, arg) {
  if (!arg) return items[0] ?? null;
  if (arg.startsWith("name:")) {
    const n = arg.slice(5);
    return items.find((x) => x.name === n && x.kind !== "workout") ?? null;
  }
  return items.find((x) => x.key === arg) ?? null;
}

const scenarioHref = (key) => `#scenarios/${encodeURIComponent(key)}`;
const scenarioHrefByName = (name) => scenarioHref(`name:${name}`);

function playingWith() {
  const ps = ui.trainer.data.profiles;
  const p = ps.profiles.find((x) => x.name === ps.active) ?? ps.profiles[0];
  return `<a class="chip-link" href="#settings/aim" title="${esc(sensText(p))} · FOV ${num(p.settings.fov)}. Change it in Settings."><span class="muted">Playing with</span> <strong>${esc(p.name)}</strong> <span>${profileCm(p).toFixed(1)} cm/360 · FOV ${num(p.settings.fov)}</span></a>`;
}

// Shown on Scenarios and Today: the trainer is open, or a message from the last action.
function trainerBanners() {
  const tr = ui.trainer;
  return `${tr.data?.playing ? `<section class="banner info"><div><div class="banner-title">The trainer is open</div><div class="banner-body">Finish or press Esc to come back. Each run is saved when its timer ends.</div></div></section>` : ""}
    ${tr.message ? `<section class="banner"><div class="banner-body">${esc(tr.message)}</div></section>` : ""}`;
}

function scenariosPage(arg) {
  const tr = ui.trainer;
  if (tr.error) return `<div class="page">${head("Play, and see how you're doing", "Scenarios")}<div class="card error">${esc(tr.error)}</div></div>`;
  if (!tr.data || !ui.insights.overview) {
    if (!tr.data) loadTrainer();
    if (!ui.insights.overview) loadInsightsOverview();
    return `<div class="empty-state">Loading scenarios…</div>`;
  }
  const all = scenarioItems();
  const item = findItem(all, arg);
  if (arg?.startsWith("name:")) {
    // Swap to the item's own address, so Back skips this hop.
    location.replace(item ? scenarioHref(item.key) : "#scenarios");
    return "";
  }
  return `<div class="page">
    ${head("Play, and see how you're doing", "Scenarios", `<span class="chips">${playingWith()}
      <button class="btn" type="button" data-action="new-workout">New workout</button>
      <button class="btn" type="button" data-action="new-scenario">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"></path></svg>New scenario</button></span>`)}
    ${trainerBanners()}
    <div class="row" style="align-items:flex-start">
      ${scenarioList(all, item)}
      <div class="detail" id="scenario-detail">${item ? itemDetail(item) : `<div class="card note-body">Nothing here yet.</div>`}</div>
    </div>
  </div>`;
}

function scenarioList(all, current) {
  const f = ui.list;
  const shown = filteredItems(all);
  const skills = [...new Set(all.map((x) => x.skill).filter(Boolean))].sort((a, b) => (a === "Workout") - (b === "Workout") || a.localeCompare(b));
  const tags = [...new Set(all.flatMap((x) => x.tags))].sort();
  const filtering = f.query || f.skill || f.tag || f.source;
  const groups = [];
  for (const x of shown) {
    const label = dayLabel(x.last);
    let g = groups.find((y) => y.label === label);
    if (!g) groups.push((g = { label, items: [] }));
    g.items.push(x);
  }
  const option = (v, label, on) => `<option value="${esc(v)}" ${on ? "selected" : ""}>${esc(label)}</option>`;
  const sub = (x) => [x.kind === "workout" ? "Workout" : x.skill, x.kind === "kovaaks" ? "KovaaK's" : x.mine ? "Yours" : "", x.last ? fmtTime(dt(x.last)) : ""].filter(Boolean).join(" · ");
  return `<section class="card list-card" aria-label="Scenarios and workouts">
    <label for="list-q" class="sr-only">Search by name</label>
    <div class="field"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9aa1ac" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"></circle><path d="M20 20l-4-4"></path></svg>
      <input id="list-q" type="search" placeholder="Search by name" value="${esc(f.query)}"></div>
    <div class="list-filters">
      <label><span>Skill</span><select class="select" data-list-filter="skill">${option("", "All skills", !f.skill)}${skills.map((s) => option(s, s, f.skill === s)).join("")}</select></label>
      <label><span>Tag</span><select class="select" data-list-filter="tag">${option("", "All tags", !f.tag)}${tags.map((t) => option(t, t, f.tag === t)).join("")}</select></label>
      <label><span>Trainer</span><select class="select" data-list-filter="source">${option("", "All", !f.source)}${option("aimadapt", "AimAdapt", f.source === "aimadapt")}${option("kovaaks", "KovaaK's", f.source === "kovaaks")}</select></label>
    </div>
    <div class="list-count small muted"><span>${filtering ? `${shown.length} of ${all.length} match` : "Newest first"}</span>
      ${filtering ? `<button type="button" class="link-btn" data-action="clear-list">Clear filters</button>` : ""}</div>
    <div class="scenario-list" id="scenario-list">${groups.map((g) => `<div class="list-day">${esc(g.label)}</div>${g.items.map((x) => `
      <a class="scenario-item ${x === current ? "on" : ""}" href="${scenarioHref(x.key)}" ${x === current ? 'aria-current="true"' : ""}>
        <span class="main"><span class="name">${esc(x.name)}</span><span class="sub">${esc(sub(x))}</span></span>
        <span class="figure">${esc(x.figure)}</span></a>`).join("")}`).join("") || `<div class="note-body">Nothing matches these filters.</div>`}</div>
  </section>`;
}

function itemDetail(x) {
  if (x.kind === "workout") return workoutDetail(x.entry);
  if (x.kind === "kovaaks") return kovaaksDetail(x.name);
  return trainerDetail(x.entry);
}

const jumpLinks = (ids) => `<nav class="jumps" aria-label="On this page">${ids.map(([id, label]) => `<button type="button" class="jump" data-action="jump" data-target="${id}">${label}</button>`).join("")}</nav>`;
const playIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4l13 8-13 8z"></path></svg>`;

function workoutDetail(e) {
  const w = e.workout;
  const playing = ui.trainer.data.playing ? "disabled" : "";
  const byId = new Map(ui.trainer.data.scenarios.map((s) => [s.id, s]));
  return `<section class="card detail-head">
      <div class="card-head" style="align-items:flex-start">
        <div><div class="eyebrow">Workout${e.builtin ? "" : " · yours"}</div><h2 class="detail-title">${esc(w.name)}</h2>
          <div class="note-body">${esc(w.description)}</div>
          <div class="small muted" style="margin-top:4px">About ${minutes(e.seconds)} · ${w.items.reduce((a, i) => a + i.runs, 0)} runs${e.last_played ? ` · last played ${esc(fmtShort(dt(e.last_played)))}` : ""}</div></div>
        <span class="chips"><a class="btn" href="#workout/${encodeURIComponent(e.id)}">${e.builtin ? "Copy &amp; edit" : "Edit"}</a>
          <button class="btn primary" type="button" data-action="play-workout" data-id="${esc(e.id)}" ${playing}>${playIcon}Start</button></span>
      </div>
      <div class="steps">${w.items.map((it, i) => {
        const s = byId.get(it.scenario);
        const name = s?.scenario?.name ?? scenarioName(it.scenario);
        return `<div class="step"><span class="mono muted">${i + 1}</span>
          ${s?.scenario ? `<a href="${scenarioHref(s.id)}">${esc(name)}</a>` : `<span>${esc(name)}</span>`}
          <span>${it.runs} run${it.runs === 1 ? "" : "s"}</span><span class="muted">${it.mode === "standard" ? "Standard" : "Adaptive"}</span></div>`;
      }).join("")}</div>
    </section>`;
}

function trainerDetail(e) {
  const s = e.scenario;
  const playing = ui.trainer.data.playing ? "disabled" : "";
  const data = ui.insights.cache.get(s.name);
  const played = (e.runs || 0) + (e.adaptive_runs || 0) > 0 || ui.insights.overview.some((o) => o.scenario === s.name);
  if (played && data === undefined) insightsFor(s.name);
  const d = data && !data.error ? data : null;
  const adaptive = d ? d.runs.filter((r) => r.adaptive && r.rating !== null && r.knob === d.knob) : [];
  const sections = d ? [
    d.curve ? curveCard(d.curve) : "",
    d.fitts ? fittsCard(d.fitts) : "",
    d.directions.length >= 3 ? directionCard(d.directions) : "",
    d.shots ? shotCard(d.shots) : "",
    d.reaction ? reactionCard(d.reaction) : "",
    d.distances.length ? distanceCard(d.distances) : "",
    d.tracking ? trackingCard(d.tracking) : "",
    d.segments.some((x) => x.success !== null) ? segmentCard(d.segments) : "",
  ].filter(Boolean) : [];
  const jumps = [["overview", "Overview"]];
  if (d) jumps.push(["work-on", "What to work on"]);
  if (sections.length) jumps.push(["insights", "Insights"]);
  if (d) jumps.push(["runs", "Runs"]);
  return `<section class="card detail-head" id="overview">
      <div class="card-head" style="align-items:flex-start">
        <div><div class="chips"><span class="tag">AimAdapt</span>${e.builtin ? "" : '<span class="tag gold">Yours</span>'}<span class="small muted">${esc(SKILL_LABELS[s.skill] ?? s.category)}</span></div>
          <h2 class="detail-title">${esc(s.name)}</h2>
          <div class="note-body">${esc(s.description || "")}</div>
          <div class="small muted" style="margin-top:4px">${esc(scenarioFacts(s))}</div></div>
        <span class="chips">
          <a class="btn" href="#edit/${encodeURIComponent(e.id)}">${e.builtin ? "Copy &amp; edit" : "Edit"}</a>
          <button class="btn" type="button" data-action="play" data-id="${esc(e.id)}" data-adaptive="0" ${playing}>Standard</button>
          <button class="btn primary" type="button" data-action="play" data-id="${esc(e.id)}" data-adaptive="1" ${playing}>${playIcon}Play adaptive</button></span>
      </div>
      ${jumpLinks(jumps)}
    </section>
    ${!played ? `<section class="card"><div class="note-body">Not played yet. Your first adaptive run is a calibration run that finds your level as you play; after that every run plays at one difficulty. See <a href="#help/adaptive">Adaptive mode</a>.</div></section>`
      : data === undefined ? `<div class="empty-state">Loading…</div>`
      : data?.error ? `<div class="card error">${esc(data.error)}</div>`
      : !d ? `<section class="card"><div class="note-body">No detailed records for this scenario yet.</div></section>`
      : `<section class="kpis" aria-label="Key figures">${insightKpis(d).slice(0, 4).join("")}</section>
        ${adaptive.length ? ratingCard(d, adaptive) : ""}
        <section class="card" id="work-on"><h2>What to work on</h2>
          ${d.notes.length ? `<div class="grid">${d.notes.map((n) => `<div class="note"><div class="note-title">${esc(n.title)}</div><div class="note-body">${esc(n.body)}</div></div>`).join("")}</div>` : `<div class="note-body">Play a few more runs for notes on what to work on.</div>`}
        </section>
        ${sections.length ? `<div id="insights" class="insights-head"><h2>Insights</h2><span class="small muted">From your last ${d.recent_runs} runs</span></div><div class="insight-cols">${sections.join("")}</div>` : ""}
        <section class="card" id="runs">${insightRunsTable(d)}</section>`}`;
}

function kovaaksDetail(name) {
  const cached = ui.scenarioCache.get(name);
  if (cached === undefined) {
    scenarioDetail(name).then(() => {
      if (route().view === "scenarios") render();
    });
    return `<div class="empty-state">Loading ${esc(name)}…</div>`;
  }
  if (!cached) return `<div class="card note-body">No runs for ${esc(name)}.</div>`;
  return `<section class="card detail-head"><div class="chips"><span class="tag blue">KovaaK's</span></div>
      <h2 class="detail-title">${esc(name)}</h2>
      <div class="small muted">Played in KovaaK's. Its stats files record score, accuracy and kill times; shot placement, reaction, tracking and replays come from runs in AimAdapt's trainer.</div>
      ${jumpLinks([["overview", "Overview"], ["work-on", "What the data says"], ["runs", "Runs"]])}
    </section>
    ${scenarioView(cached)}`;
}

// Keeps the list where it was scrolled when the page redraws for a new selection.
function afterScenarios() {
  const el = document.getElementById("scenario-list");
  if (!el) return;
  el.scrollTop = ui.list.scroll;
  el.addEventListener("scroll", () => (ui.list.scroll = el.scrollTop), { passive: true });
}

document.addEventListener("click", (e) => {
  const btn = e.target.closest("[data-action]");
  if (!btn) return;
  if (btn.dataset.action === "jump") document.getElementById(btn.dataset.target)?.scrollIntoView({ behavior: "smooth", block: "start" });
  else if (btn.dataset.action === "clear-list") {
    Object.assign(ui.list, { query: "", skill: "", tag: "", source: "" });
    render();
  }
});

document.addEventListener("input", (e) => {
  if (e.target.id !== "list-q") return;
  ui.list.query = e.target.value;
  const pos = e.target.selectionStart;
  render();
  const q = document.getElementById("list-q");
  q.focus();
  q.setSelectionRange(pos, pos);
});

document.addEventListener("change", (e) => {
  const key = e.target.dataset.listFilter;
  if (key) {
    ui.list[key] = e.target.value;
    render();
  }
});
