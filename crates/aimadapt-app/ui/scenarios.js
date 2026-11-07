// Scenarios: everything you can play or have played (the trainer's scenarios, your
// workouts and your KovaaK's scenarios) in four tabs (Recent, Browse, Workouts,
// Yours, plus Hidden), beside the selected one. The selection is one scrolling page:
// Play at the top, then key figures, the rating chart, what to work on, Insights
// and the runs. Favorites and hidden items are saved with `save_marks`.
//
// Loaded after app.js, trainer.js, insights.js and program.js; it draws with their
// helpers and data (ui.trainer.data, ui.insights, ui.snap).

// tab is null until the page first draws: Recent if you've played anything, else Browse.
ui.list = { tab: null, query: "", skill: "", tags: [], source: "", sort: "skill", moreTags: false, scroll: 0 };

const LIST_TABS = [["recent", "Recent"], ["browse", "Browse"], ["workouts", "Workouts"], ["yours", "Yours"]];
const LIST_SORTS = [["skill", "By skill"], ["name", "By name"], ["most", "Most played"], ["stale", "Longest since played"]];
// Tags shown before "More tags".
const TAGS_SHOWN = 8;

// Built-in workouts for the program's days live on Today and Progress, not here.
const isProgramDay = (e) => e.builtin && /^builtin:core-/.test(e.id);

const marks = () => ui.trainer.data?.marks ?? { favorites: [], hidden: [] };
const isFavorite = (key) => marks().favorites.includes(key);
const isHidden = (key) => marks().hidden.includes(key);

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
      plays: (e.runs || 0) + (e.adaptive_runs || 0),
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
      plays: 0,
      figure: `${e.workout.items.reduce((a, i) => a + i.runs, 0)} runs`,
      mine: !e.builtin,
      entry: e,
    });
  }
  for (const s of ui.snap.scenarios) {
    if (names.has(s.scenario)) continue;
    items.push({ key: `kovaaks:${s.scenario}`, kind: "kovaaks", name: s.scenario, skill: "", tags: [], last: s.last_played, plays: s.runs ?? 0, figure: num(s.best_score), entry: s });
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

// The tab that shows first: Recent once you've played something.
function listTab(all) {
  if (!ui.list.tab) ui.list.tab = all.some((x) => x.last && !isHidden(x.key)) ? "recent" : "browse";
  return ui.list.tab;
}

// What a tab holds before its filters. Hidden items only show on Hidden.
function tabItems(all, tab) {
  if (tab === "hidden") return all.filter((x) => isHidden(x.key));
  const shown = all.filter((x) => !isHidden(x.key));
  if (tab === "recent") return shown.filter((x) => x.last);
  if (tab === "browse") return shown.filter((x) => x.kind === "trainer");
  if (tab === "workouts") return shown.filter((x) => x.kind === "workout");
  return shown.filter((x) => x.mine);
}

// The filters each tab offers; the name search is on every tab.
const tabFilters = (tab) => ({ skill: tab === "browse", tags: tab === "browse", sort: tab === "browse", source: tab === "recent" });

function filteredItems(items, tab) {
  const f = ui.list;
  const on = tabFilters(tab);
  const q = f.query.trim().toLowerCase();
  return items.filter((x) =>
    (!q || x.name.toLowerCase().includes(q)) &&
    (!on.skill || !f.skill || x.skill === f.skill) &&
    (!on.tags || f.tags.every((t) => x.tags.includes(t))) &&
    (!on.source || !f.source || (f.source === "kovaaks" ? x.kind === "kovaaks" : x.kind !== "kovaaks")));
}

const isFiltering = (tab) => {
  const f = ui.list;
  const on = tabFilters(tab);
  return Boolean(f.query.trim() || (on.skill && f.skill) || (on.tags && f.tags.length) || (on.source && f.source));
};

const skillOrder = (label) => {
  const i = Object.values(SKILL_LABELS).indexOf(label);
  return i < 0 ? 99 : i;
};
const byName = (a, b) => a.name.localeCompare(b.name);

// The shown items in labelled groups, favorites first except on Recent (by day).
function listGroups(items, tab) {
  if (tab === "recent") {
    const groups = [];
    for (const x of items) {
      const label = dayLabel(x.last);
      let g = groups.find((y) => y.label === label);
      if (!g) groups.push((g = { label, items: [] }));
      g.items.push(x);
    }
    return groups;
  }
  const favorites = marks().favorites;
  const fav = items.filter((x) => isFavorite(x.key)).sort((a, b) => favorites.indexOf(a.key) - favorites.indexOf(b.key));
  const rest = items.filter((x) => !isFavorite(x.key));
  const groups = fav.length && tab !== "hidden" ? [{ label: "Favorites", items: fav }] : [];
  const others = tab === "hidden" ? items : rest;
  if (tab === "browse") {
    const sort = ui.list.sort;
    if (sort === "skill") {
      for (const x of [...others].sort((a, b) => skillOrder(a.skill) - skillOrder(b.skill) || byName(a, b))) {
        const label = x.skill || "Other";
        let g = groups.find((y) => y.label === label);
        if (!g) groups.push((g = { label, items: [] }));
        g.items.push(x);
      }
      return groups;
    }
    const cmp = sort === "most" ? (a, b) => b.plays - a.plays || byName(a, b)
      : sort === "stale" ? (a, b) => (!a.last ? (b.last ? -1 : byName(a, b)) : !b.last ? 1 : a.last < b.last ? -1 : a.last > b.last ? 1 : byName(a, b))
      : byName;
    if (others.length) groups.push({ label: fav.length ? "Everything else" : "All scenarios", items: [...others].sort(cmp) });
    return groups;
  }
  if (tab === "yours") {
    const sc = others.filter((x) => x.kind !== "workout");
    const wo = others.filter((x) => x.kind === "workout");
    if (sc.length) groups.push({ label: "Scenarios", items: sc });
    if (wo.length) groups.push({ label: "Workouts", items: wo });
    return groups;
  }
  if (others.length) groups.push({ label: tab === "hidden" ? "Hidden" : fav.length ? "Everything else" : "", items: others });
  return groups;
}

// The item a route argument names, or the first one the tab shows. "name:<scenario>"
// (links from older pages and from Today) finds it by name.
function findItem(items, arg, shown) {
  if (!arg) return shown[0] ?? null;
  if (arg.startsWith("name:")) {
    const n = arg.slice(5);
    return items.find((x) => x.name === n && x.kind !== "workout") ?? null;
  }
  return items.find((x) => x.key === arg) ?? null;
}

const scenarioHref = (key) => `#scenarios/${encodeURIComponent(key)}`;
const scenarioHrefByName = (name) => scenarioHref(`name:${name}`);

// Opens Scenarios on a tab, optionally filtered to one skill (Today's links).
function openScenariosTab(tab, skill = "") {
  Object.assign(ui.list, { tab, query: "", skill, tags: [], source: "", scroll: 0 });
  if (location.hash === "#scenarios") render();
  else location.hash = "#scenarios";
}

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
  const tab = listTab(all);
  const shown = filteredItems(tabItems(all, tab), tab);
  const groups = listGroups(shown, tab);
  const item = findItem(all, arg, groups.flatMap((g) => g.items));
  if (arg?.startsWith("name:")) {
    // Swap to the item's own address, so Back skips this hop.
    location.replace(item ? scenarioHref(item.key) : "#scenarios");
    return "";
  }
  if (!arg && item) {
    // Open the tab's first item by its address, so it stays open while the list
    // changes beside it (Favorite and Hide act on it, not on whatever is first).
    location.replace(scenarioHref(item.key));
    return "";
  }
  return `<div class="page">
    ${head("Play, and see how you're doing", "Scenarios", `<span class="chips">${playingWith()}
      <button class="btn" type="button" data-action="new-workout">New workout</button>
      <button class="btn" type="button" data-action="new-scenario">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"></path></svg>New scenario</button></span>`)}
    ${trainerBanners()}
    <div class="row" style="align-items:flex-start">
      ${scenarioList(all, tab, shown, groups, item)}
      <div class="detail" id="scenario-detail">${item ? itemDetail(item) : `<div class="card note-body">${tab === "browse" || tab === "recent" ? "Nothing here yet." : "Pick something from the list."}</div>`}</div>
    </div>
  </div>`;
}

const starIcon = (filled) => `<svg width="14" height="14" viewBox="0 0 24 24" fill="${filled ? "currentColor" : "none"}" stroke="currentColor" stroke-width="2" stroke-linejoin="round" aria-hidden="true"><path d="M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z"></path></svg>`;

const TAB_NOTES = {
  recent: "Newest first",
  browse: "Everything you can play in the trainer",
  workouts: "Built-in workouts and yours",
  yours: "Scenarios and workouts you made or copied",
  hidden: "Hidden from the other tabs",
};

function emptyTab(tab) {
  if (tab === "recent") return `Nothing played yet. <button type="button" class="link-btn" data-action="list-tab" data-tab="browse">Browse</button> has every scenario you can play.`;
  if (tab === "yours") return "Nothing of yours yet. Use Copy &amp; edit on any scenario or workout, or New scenario above.";
  if (tab === "hidden") return "Nothing hidden.";
  return "Nothing here yet.";
}

function scenarioList(all, tab, shown, groups, current) {
  const f = ui.list;
  const on = tabFilters(tab);
  const base = tabItems(all, tab);
  const filtering = isFiltering(tab);
  const hidden = all.filter((x) => isHidden(x.key)).length;
  const option = (v, label, sel) => `<option value="${esc(v)}" ${sel ? "selected" : ""}>${esc(label)}</option>`;
  const sub = (x) => [x.kind === "workout" ? "Workout" : x.skill, x.kind === "kovaaks" ? "KovaaK's history" : x.mine ? "Yours" : "", x.last && tab === "recent" ? fmtTime(dt(x.last)) : ""].filter(Boolean).join(" · ");

  let filters = "";
  if (on.skill || on.sort || on.source) {
    const skills = [...new Set(base.map((x) => x.skill).filter(Boolean))].sort((a, b) => skillOrder(a) - skillOrder(b));
    filters = `<div class="list-filters">
      ${on.skill ? `<label><span>Skill</span><select class="select" data-list-filter="skill">${option("", "All skills", !f.skill)}${skills.map((s) => option(s, s, f.skill === s)).join("")}</select></label>` : ""}
      ${on.sort ? `<label><span>Sort</span><select class="select" data-list-filter="sort">${LIST_SORTS.map(([v, l]) => option(v, l, f.sort === v)).join("")}</select></label>` : ""}
      ${on.source ? `<label><span>Trainer</span><select class="select" data-list-filter="source">${option("", "All", !f.source)}${option("aimadapt", "AimAdapt", f.source === "aimadapt")}${option("kovaaks", "KovaaK's", f.source === "kovaaks")}</select></label>` : ""}
    </div>`;
  }
  let tagChips = "";
  if (on.tags) {
    const count = new Map();
    for (const x of base) for (const t of x.tags) count.set(t, (count.get(t) ?? 0) + 1);
    const tags = [...count.keys()].sort((a, b) => count.get(b) - count.get(a) || a.localeCompare(b));
    const visible = f.moreTags ? tags : tags.filter((t, i) => i < TAGS_SHOWN || f.tags.includes(t));
    if (tags.length) {
      tagChips = `<div class="tag-chips" role="group" aria-label="Tags">${visible.map((t) => `<button type="button" class="chip ${f.tags.includes(t) ? "on" : ""}" data-action="list-tag" data-tag="${esc(t)}" aria-pressed="${f.tags.includes(t)}">${esc(t)}</button>`).join("")}
        ${tags.length > TAGS_SHOWN ? `<button type="button" class="link-btn" data-action="more-tags">${f.moreTags ? "Fewer tags" : `${tags.length - visible.length} more`}</button>` : ""}</div>`;
    }
  }

  return `<section class="card list-card" aria-label="Scenarios and workouts">
    <div class="list-tabs" role="tablist" aria-label="Lists">${LIST_TABS.map(([id, label]) => `<button type="button" role="tab" class="list-tab ${tab === id ? "on" : ""}" aria-selected="${tab === id}" data-action="list-tab" data-tab="${id}">${label}</button>`).join("")}</div>
    <label for="list-q" class="sr-only">Search by name</label>
    <div class="field"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9aa1ac" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"></circle><path d="M20 20l-4-4"></path></svg>
      <input id="list-q" type="search" placeholder="Search by name" value="${esc(f.query)}"></div>
    ${filters}${tagChips}
    <div class="list-count small muted"><span>${filtering ? `${shown.length} of ${base.length} match` : TAB_NOTES[tab]}</span>
      ${filtering ? `<button type="button" class="link-btn" data-action="clear-list">Clear filters</button>` : ""}</div>
    <div class="scenario-list" id="scenario-list">${groups.map((g) => `${g.label ? `<div class="list-day">${esc(g.label)}</div>` : ""}${g.items.map((x) => `
      <a class="scenario-item ${x === current ? "on" : ""}" href="${scenarioHref(x.key)}" ${x === current ? 'aria-current="true"' : ""}>
        <span class="main"><span class="name">${esc(x.name)}</span><span class="sub">${esc(sub(x))}</span></span>
        ${isFavorite(x.key) && tab === "recent" ? `<span class="fav-mark" title="Favorite">${starIcon(true)}</span>` : ""}
        <span class="figure">${esc(x.figure)}</span></a>`).join("")}`).join("") || `<div class="note-body">${filtering ? "Nothing matches these filters." : emptyTab(tab)}</div>`}</div>
    ${tab === "hidden" ? `<button type="button" class="link-btn" data-action="list-tab" data-tab="recent">Back to the lists</button>`
      : hidden ? `<button type="button" class="link-btn" data-action="list-tab" data-tab="hidden">${hidden} hidden</button>` : ""}
  </section>`;
}

// Favorite and Hide, at the top of every item's page.
function markButtons(key) {
  const fav = isFavorite(key);
  const hid = isHidden(key);
  return `<button class="btn icon-btn ${fav ? "on" : ""}" type="button" data-action="favorite" data-key="${esc(key)}" aria-pressed="${fav}" title="${fav ? "Remove from favorites" : "Add to favorites"}" aria-label="Favorite">${starIcon(fav)}</button>
    <button class="btn" type="button" data-action="hide" data-key="${esc(key)}" title="${hid ? "Show it on the lists again" : "Keep it off the lists. Its runs stay, and the Hidden list has it."}">${hid ? "Show again" : "Hide"}</button>`;
}

async function toggleMark(list, key) {
  const tr = ui.trainer;
  const m = structuredClone(marks());
  const has = m[list].includes(key);
  m[list] = has ? m[list].filter((k) => k !== key) : [...m[list], key];
  const before = tr.data.marks;
  tr.data.marks = m;
  render();
  try {
    tr.data.marks = await invoke("save_marks", { marks: m });
  } catch (e) {
    tr.data.marks = before;
    tr.message = `Couldn't save: ${e}`;
  }
  render();
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
        <div><div class="eyebrow">Workout${e.builtin ? "" : " · yours"}${isHidden(`workout:${e.id}`) ? " · hidden" : ""}</div><h2 class="detail-title">${esc(w.name)}</h2>
          <div class="note-body">${esc(w.description)}</div>
          <div class="small muted" style="margin-top:4px">About ${minutes(e.seconds)} · ${w.items.reduce((a, i) => a + i.runs, 0)} runs${e.last_played ? ` · last played ${esc(fmtShort(dt(e.last_played)))}` : ""}</div></div>
        <span class="chips">${markButtons(`workout:${e.id}`)}<a class="btn" href="#workout/${encodeURIComponent(e.id)}">${e.builtin ? "Copy &amp; edit" : "Edit"}</a>
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
        <div><div class="chips"><span class="tag">AimAdapt</span>${e.builtin ? "" : '<span class="tag gold">Yours</span>'}${isHidden(e.id) ? '<span class="tag grey">Hidden</span>' : ""}<span class="small muted">${esc(SKILL_LABELS[s.skill] ?? s.category)}</span></div>
          <h2 class="detail-title">${esc(s.name)}</h2>
          <div class="note-body">${esc(s.description || "")}</div>
          <div class="small muted" style="margin-top:4px">${esc(scenarioFacts(s))}</div></div>
        <span class="chips">${markButtons(e.id)}
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

// Each KovaaK's scenario's .sce file and whether the trainer can play it, by name.
ui.sceInfo = new Map();

function loadSceInfo(name) {
  ui.sceInfo.set(name, null);
  invoke("sce_info", { name })
    .then((info) => ui.sceInfo.set(name, info))
    .catch((e) => ui.sceInfo.set(name, { error: String(e) }))
    .then(() => {
      if (route().view === "scenarios") render();
    });
}

// "Play in AimAdapt" for a KovaaK's scenario whose file the trainer can play.
function sceCard(name) {
  if (!ui.sceInfo.has(name)) loadSceInfo(name);
  const s = ui.sceInfo.get(name);
  if (!s || s.error || !s.path) return "";
  const playing = ui.trainer.data.playing ? "disabled" : "";
  const history = s.runs
    ? `In AimAdapt: ${s.runs} run${s.runs === 1 ? "" : "s"}, best ${num(s.best)}${s.last_played ? `, last ${fmtShort(dt(s.last_played))}` : ""}.`
    : "";
  if (!s.playable) {
    return `<section class="card"><div class="note-body"><strong>Can't play this in AimAdapt yet.</strong> It uses ${esc(s.reasons.join("; "))}. See <a href="#help/kovaaks-play">Playing KovaaK's scenarios</a>.</div>${history ? `<div class="small muted">${esc(history)}</div>` : ""}</section>`;
  }
  return `<section class="card sce-card"><div class="card-head" style="align-items:flex-start">
      <div><h2>Play in AimAdapt</h2>
        <div class="note-body">Plays this scenario's own file on its map in AimAdapt's trainer, in standard mode. Its runs are kept apart from your KovaaK's runs.</div>
        ${s.notes.length ? `<ul class="small muted sce-notes">${s.notes.map((n) => `<li>${esc(n[0].toUpperCase() + n.slice(1))}.</li>`).join("")}</ul>` : ""}
        ${history ? `<div class="small muted">${esc(history)}</div>` : ""}</div>
      <button class="btn primary" type="button" data-action="play-sce" data-path="${esc(s.path)}" ${playing}>${playIcon}Play</button>
    </div></section>`;
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
  const key = `kovaaks:${name}`;
  return `<section class="card detail-head">
      <div class="card-head" style="align-items:flex-start">
        <div><div class="chips"><span class="tag blue">KovaaK's</span>${isHidden(key) ? '<span class="tag grey">Hidden</span>' : ""}</div>
          <h2 class="detail-title">${esc(name)}</h2></div>
        <span class="chips">${markButtons(key)}</span>
      </div>
      <div class="small muted">Played in KovaaK's. Its stats files record score, accuracy and kill times; shot placement, reaction, tracking and replays come from runs in AimAdapt's trainer.</div>
      ${jumpLinks([["overview", "Overview"], ["work-on", "What the data says"], ["runs", "Runs"]])}
    </section>
    ${sceCard(name)}
    ${scenarioView(cached)}`;
}

async function startSce(path) {
  try {
    await invoke("play_sce", { path });
    ui.trainer.message = null;
  } catch (e) {
    ui.trainer.message = String(e);
  }
  await loadTrainer();
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
  const a = btn.dataset.action;
  if (a === "jump") document.getElementById(btn.dataset.target)?.scrollIntoView({ behavior: "smooth", block: "start" });
  else if (a === "clear-list") {
    Object.assign(ui.list, { query: "", skill: "", tags: [], source: "" });
    render();
  } else if (a === "list-tab") {
    // The open scenario stays open; the list changes beside it.
    if (ui.list.tab !== btn.dataset.tab) Object.assign(ui.list, { tab: btn.dataset.tab, scroll: 0 });
    render();
  } else if (a === "list-tag") {
    const t = btn.dataset.tag;
    ui.list.tags = ui.list.tags.includes(t) ? ui.list.tags.filter((x) => x !== t) : [...ui.list.tags, t];
    render();
  } else if (a === "more-tags") {
    ui.list.moreTags = !ui.list.moreTags;
    render();
  } else if (a === "favorite") toggleMark("favorites", btn.dataset.key);
  else if (a === "hide") toggleMark("hidden", btn.dataset.key);
  else if (a === "browse-skill") openScenariosTab("browse", btn.dataset.skill ?? "");
  else if (a === "list-tab-open") openScenariosTab(btn.dataset.tab);
  else if (a === "play-sce") startSce(btn.dataset.path);
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
