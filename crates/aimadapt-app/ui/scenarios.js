// Scenarios: everything you can play or have played (the trainer's scenarios, your
// workouts and your KovaaK's scenarios).
//
// - #scenarios/<tab> is a list across the whole page, one tab each for Browse and
//   Workouts (plus Hidden): a search, the tab's filters, and a table you can sort by
//   any column, a hundred rows at a time. Recent and Yours are Browse's Played and
//   Yours filters now; their old addresses open Browse with the filter set (see
//   OLD_TABS). Program (the week's
//   plan, drawn by programTab in program.js) sits in the same row of tabs but is not
//   a list: it has no search, filters or table.
// - #scenarios/<key> is one item's own page: Play at the top, then key figures, the
//   rating chart, what to work on, Insights and the runs.
//
// Favorites and hidden items are saved with `save_marks`. Loaded after app.js,
// trainer.js, insights.js and program.js; it draws with their helpers and data
// (ui.trainer.data, ui.insights, ui.snap).

// The row of tabs: the one list of what Scenarios has. Hidden is added when something is hidden.
const SCENARIO_TABS = [["browse", "Browse"], ["workouts", "Workouts"], ["program", "Program"]];
// Addresses from when Recent and Yours were tabs: each opens Browse with one filter set.
const OLD_TABS = { recent: { played: "yes" }, yours: { owner: "mine" } };
const TAB_LABELS = Object.fromEntries([...SCENARIO_TABS, ["hidden", "Hidden"]]);
const isScenarioTab = (arg) => Object.hasOwn(TAB_LABELS, arg ?? "");
// A tab that is a list (has TAB_SETUP, filters and a table): every tab but Program.
const isListTab = (arg) => isScenarioTab(arg) && arg !== "program";
// Rows shown before "Show more", and tags before "N more".
const PAGE_ROWS = 100;
const TAGS_SHOWN = 10;

// Each tab keeps its own filters, sort, rows shown and scroll position. tab is the
// last list you were on: the sidebar link and an item's back link go to it.
const NO_FILTERS = { query: "", skill: "", tags: [], kind: "", played: "", owner: "", favorites: false };
ui.list = { tab: null, filters: {}, sort: {}, limit: {}, scroll: {}, moreTags: false };
const listFilters = (tab) => (ui.list.filters[tab] ??= structuredClone(NO_FILTERS));

// Built-in workouts for the program's days live on Today and Scenarios › Program, not in the lists.
const isProgramDay = (e) => e.builtin && /^builtin:core-/.test(e.id);

const marks = () => ui.trainer.data?.marks ?? { favorites: [], hidden: [] };
const isFavorite = (key) => marks().favorites.includes(key);
const isHidden = (key) => marks().hidden.includes(key);

// Your KovaaK's scenario files (sce_library): null until asked for, then
// "loading", then the list (empty if KovaaK's isn't installed).
ui.sceLibrary = null;

function loadSceLibrary() {
  ui.sceLibrary = "loading";
  invoke("sce_library")
    .catch(() => [])
    .then((list) => {
      ui.sceLibrary = list;
      if (["scenarios", "today"].includes(route().view)) render();
    });
}

// KovaaK's AimTypeTag as a skill: "Target Switching" reads "Target switching".
const aimTypeLabel = (t) => (t ? t[0].toUpperCase() + t.slice(1).toLowerCase() : "");

// Everything in the lists, newest first; never-played items last, by name.
function scenarioItems() {
  if (ui.sceLibrary === null) loadSceLibrary();
  const files = new Map((Array.isArray(ui.sceLibrary) ? ui.sceLibrary : []).map((f) => [f.name.toLowerCase(), f]));
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
    const rated = e.rating !== null && e.rating !== undefined;
    items.push({
      key: e.id,
      kind: "trainer",
      name: s.name,
      skill: SKILL_LABELS[s.skill] ?? "",
      tags: s.tags ?? [],
      description: s.description ?? "",
      last: later(e.last_played, o?.last_played ?? null),
      plays: (e.runs || 0) + (e.adaptive_runs || 0),
      rating: rated ? e.rating : null,
      knob: e.knob ?? null,
      best: e.runs ? e.best ?? null : null,
      figure: rated ? fmtKnob(e.knob, e.rating) : e.runs ? num(e.best) : "",
      mine: !e.builtin,
      entry: e,
    });
  }
  for (const e of d.workouts) {
    if (!e.workout || isProgramDay(e)) continue;
    const runs = e.workout.items.reduce((a, i) => a + i.runs, 0);
    items.push({
      key: `workout:${e.id}`,
      kind: "workout",
      name: e.workout.name,
      skill: "",
      tags: [],
      description: e.workout.description ?? "",
      last: e.last_played,
      plays: null,
      rating: null,
      best: null,
      steps: e.workout.items.length,
      runs,
      seconds: e.seconds,
      figure: `${runs} runs`,
      mine: !e.builtin,
      entry: e,
    });
  }
  // KovaaK's scenarios: the ones you've played (from its stats files) and the
  // files in your KovaaK's and Workshop folders, once each.
  const kovaaks = (name, sce, s) => ({
    key: `kovaaks:${name}`, kind: "kovaaks", name, skill: aimTypeLabel(sce?.aim_type), tags: sce?.tags ?? [], description: sce?.description ?? "",
    last: s?.last_played ?? null, plays: s?.runs ?? 0, rating: null, best: s?.best_score ?? null, figure: s ? num(s.best_score) : "", mine: false, sce, entry: s ?? null,
  });
  for (const s of ui.snap.scenarios) {
    if (names.has(s.scenario)) continue;
    names.add(s.scenario);
    items.push(kovaaks(s.scenario, files.get(s.scenario.toLowerCase()), s));
  }
  const lower = new Set([...names].map((n) => n.toLowerCase()));
  for (const f of files.values()) if (!lower.has(f.name.toLowerCase())) items.push(kovaaks(f.name, f, null));
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

// The tab Scenarios opens on: the last one you were on, else Browse.
const defaultTab = () => (isScenarioTab(ui.list.tab) ? ui.list.tab : "browse");

// What a tab holds before its filters. Hidden items only show on Hidden.
function tabItems(all, tab) {
  if (tab === "hidden") return all.filter((x) => isHidden(x.key));
  const shown = all.filter((x) => !isHidden(x.key));
  if (tab === "browse") return shown.filter((x) => x.kind !== "workout");
  return shown.filter((x) => x.kind === "workout");
}

const skillOrder = (label) => {
  const i = Object.values(SKILL_LABELS).indexOf(label);
  return i < 0 ? 99 : i;
};
const byName = (a, b) => a.name.localeCompare(b.name);
const kindLabel = (x) => (x.kind === "workout" ? "Workout" : x.kind === "kovaaks" ? "KovaaK's" : "AimAdapt");
const blank = (v) => v === null || v === undefined || v === "";
// Ascending for d = 1, descending for d = -1; blanks always last.
const byKey = (x, y, d) => (blank(x) ? (blank(y) ? 0 : 1) : blank(y) ? -1 : x < y ? -d : x > y ? d : 0);
// Ratings only compare within one kind (seconds, meters, degrees a second), so
// they sort by kind, then best first.
function byRating(a, b, d) {
  if (blank(a.rating) || blank(b.rating)) return byKey(a.rating, b.rating, d);
  if (a.knob !== b.knob) return a.knob < b.knob ? -1 : 1;
  const v = (x) => (higherIsHarder(x.knob) ? x.rating : -x.rating);
  return (v(a) - v(b)) * d;
}

// The "Rating or score" column: ratings first (by kind, then best first, as byRating),
// then best scores, then rows with neither. The order of those three groups stays put
// whichever way the column is sorted, so blanks are always last.
const figureGroup = (x) => (!blank(x.rating) ? 0 : !blank(x.best) ? 1 : 2);
function byFigure(a, b, d) {
  const ga = figureGroup(a);
  const gb = figureGroup(b);
  if (ga !== gb) return ga - gb;
  if (ga === 0) return byRating(a, b, d);
  if (ga === 1) return (a.best - b.best) * d;
  return 0;
}

// What a table row's last-played cell says: the day, with the time today and
// yesterday.
function lastText(x) {
  const label = dayLabel(x.last);
  return label === "Today" || label === "Yesterday" ? `${label}, ${fmtTime(dt(x.last))}` : label;
}

const starIcon = (filled) => `<svg width="14" height="14" viewBox="0 0 24 24" fill="${filled ? "currentColor" : "none"}" stroke="currentColor" stroke-width="2" stroke-linejoin="round" aria-hidden="true"><path d="M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z"></path></svg>`;
const playIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4l13 8-13 8z"></path></svg>`;
const dash = `<span class="dim">–</span>`;

// Play (an adaptive run) for trainer scenarios, Start session for workouts, and
// Play in AimAdapt (its own file, in standard mode) for a KovaaK's scenario the
// trainer can play.
function playButton(x) {
  const playing = ui.trainer.data.playing ? "disabled" : "";
  if (x.kind === "trainer") return `<button class="btn play-btn" type="button" data-action="play" data-id="${esc(x.key)}" data-adaptive="1" ${playing} aria-label="Play ${esc(x.name)}">${playIcon}Play</button>`;
  if (x.kind === "workout") return `<button class="btn play-btn" type="button" data-action="play-workout" data-id="${esc(x.entry.id)}" ${playing} aria-label="Start session ${esc(x.name)}">${playIcon}Start session</button>`;
  if (x.sce?.playable) return `<button class="btn play-btn" type="button" data-action="play-sce" data-path="${esc(x.sce.path)}" ${playing} aria-label="Play ${esc(x.name)} in AimAdapt" title="Plays this scenario's own file in AimAdapt's trainer">${playIcon}Play in AimAdapt</button>`;
  return "";
}

function starButton(x) {
  const on = isFavorite(x.key);
  return `<button type="button" class="star-btn ${on ? "on" : ""}" data-action="favorite" data-key="${esc(x.key)}" aria-pressed="${on}" aria-label="Favorite ${esc(x.name)}" title="${on ? "Remove from favorites" : "Add to favorites"}">${starIcon(on)}</button>`;
}

function nameCell(x) {
  const sub = x.kind === "workout" ? x.description : x.tags.join(" · ");
  return `<a class="row-name" href="${scenarioHref(x.key)}">${esc(x.name)}</a>${x.mine && x.kind === "trainer" ? ' <span class="tag gold">Yours</span>' : ""}
    ${sub ? `<div class="row-sub" title="${esc(sub)}">${esc(sub)}</div>` : ""}`;
}

// Table columns. key (or cmp) makes a column sortable; dir is the order a first
// click sorts in. Sorted by a column with group, the table has a heading row for
// each group (each skill, say), so a long list is easier to scan.
const COLUMNS = {
  fav: { sr: "Favorite", cls: "star-col", cell: starButton },
  name: { label: "Name", cls: "name-col", dir: 1, key: (x) => x.name.toLowerCase(), cell: nameCell },
  kind: {
    label: "Type", dir: 1, key: kindLabel, group: kindLabel,
    cell: (x) => `<span class="tag ${x.kind === "kovaaks" ? "blue" : x.kind === "workout" ? "grey" : ""}">${kindLabel(x)}</span>${x.sce?.playable ? ' <span class="small muted" title="Its file plays in AimAdapt\'s trainer">plays here</span>' : ""}`,
  },
  skill: { label: "Skill", dir: 1, key: (x) => (x.skill ? `${String(skillOrder(x.skill)).padStart(2, "0")} ${x.skill}` : null), group: (x) => x.skill || "No skill", cell: (x) => esc(x.skill) },
  figure: {
    label: "Rating or score", cls: "r", dir: -1, cmp: byFigure,
    group: (x) => (figureGroup(x) === 0 ? `Rated on ${knobWord(x.knob)}` : figureGroup(x) === 1 ? "Best score" : "No rating or score yet"),
    title: "Your 85% rating on AimAdapt scenarios, or your best score where there is no rating. Sorts by kind, then best first.",
    cell: (x) => (figureGroup(x) === 0 ? `<span class="num" title="85% ${esc(knobWord(x.knob))}">${esc(fmtKnob(x.knob, x.rating))}</span>` : figureGroup(x) === 1 ? `<span class="mono" title="Best score">${num(x.best)}</span>` : dash),
  },
  plays: { label: "Runs", cls: "r", dir: -1, key: (x) => x.plays, cell: (x) => (x.plays ? `<span class="mono">${x.plays}</span>` : dash) },
  last: { label: "Last played", dir: -1, key: (x) => x.last, group: (x) => (x.last ? dayLabel(x.last) : "Never played"), cell: (x) => (x.last ? `<span title="${esc(`${fmtShort(dt(x.last))}, ${fmtTime(dt(x.last))}`)}">${esc(lastText(x))}</span>` : `<span class="dim">Never</span>`) },
  owner: { label: "Made by", dir: 1, key: (x) => (x.mine ? 0 : 1), group: (x) => (x.mine ? "Yours" : "Built-in"), cell: (x) => (x.mine ? '<span class="tag gold">Yours</span>' : '<span class="muted">Built-in</span>') },
  steps: { label: "Scenarios", cls: "r", dir: -1, key: (x) => x.steps, cell: (x) => `<span class="mono">${x.steps}</span> <span class="dim">· ${x.runs} runs</span>` },
  length: { label: "Length", cls: "r", dir: 1, key: (x) => x.seconds, cell: (x) => `About ${minutes(x.seconds)}` },
  play: { sr: "Play", cls: "r", cell: playButton },
  unhide: { sr: "Show again", cls: "r", cell: (x) => `<button class="btn play-btn" type="button" data-action="hide" data-key="${esc(x.key)}">Show again</button>` },
};

// What each tab shows: its columns, its first sort and its filters. The search is
// on every tab, and Favorites on every tab but Hidden. "mine" is the Yours chip (it
// sets the owner filter); "owner" is the Made by select.
const TAB_SETUP = {
  browse: { cols: ["fav", "name", "kind", "skill", "figure", "plays", "last", "play"], sort: ["skill", 1], filters: ["kind", "skill", "played", "tags", "mine"], note: "The trainer's scenarios and your KovaaK's scenarios" },
  workouts: { cols: ["fav", "name", "owner", "steps", "length", "last", "play"], sort: ["name", 1], filters: ["owner"], note: "Built-in workouts and yours" },
  hidden: { cols: ["name", "kind", "skill", "plays", "last", "unhide"], sort: ["name", 1], filters: ["kind"], note: "Kept off the other tabs. Their runs stay." },
};

// What a tab is sorted by, as [column, direction]: the sort you picked, else its first
// sort. Browse opens by Last played (newest first, so rows group by day with the never
// played ones last) once anything in it has been played, and by skill before that.
// base is the tab's rows before filters. Everything that draws or acts on the sort
// asks here.
function sortOf(tab, base) {
  const picked = ui.list.sort[tab];
  if (picked) return picked;
  if (tab === "browse" && base.some((x) => x.last)) return ["last", -1];
  return TAB_SETUP[tab].sort;
}

// Every word of the search has to be in the name, skill, tags or description.
function matches(x, words) {
  if (!words.length) return true;
  const text = `${x.name} ${x.skill} ${x.tags.join(" ")} ${x.description}`.toLowerCase();
  return words.every((w) => text.includes(w));
}

function filterRows(items, tab) {
  const f = listFilters(tab);
  const on = new Set(TAB_SETUP[tab].filters);
  const words = f.query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return items.filter((x) =>
    matches(x, words) &&
    (!on.has("skill") || !f.skill || x.skill === f.skill) &&
    (!on.has("kind") || !f.kind || (f.kind === "kovaaks-play" ? Boolean(x.sce?.playable) : x.kind === f.kind)) &&
    (!on.has("played") || !f.played || (f.played === "yes" ? Boolean(x.last) : !x.last)) &&
    (!(on.has("owner") || on.has("mine")) || !f.owner || (f.owner === "mine") === x.mine) &&
    (!on.has("tags") || f.tags.every((t) => x.tags.includes(t))) &&
    (tab === "hidden" || !f.favorites || isFavorite(x.key)));
}

function isFiltering(tab) {
  const f = listFilters(tab);
  const on = new Set(TAB_SETUP[tab].filters);
  return Boolean(f.query.trim() || f.favorites || [...on].some((k) => (k === "tags" ? f.tags.length : f[k === "mine" ? "owner" : k])));
}

function sortRows(rows, sort) {
  const [id, dir] = sort;
  const c = COLUMNS[id];
  const cmp = c.cmp ?? ((a, b, d) => byKey(c.key(a), c.key(b), d));
  return [...rows].sort((a, b) => cmp(a, b, dir) || byName(a, b));
}

// An item's page. A tab other than Overview adds its id; Overview is the bare address.
const scenarioHref = (key, tab = "overview") => `#scenarios/${encodeURIComponent(key)}${tab && tab !== "overview" ? `/${tab}` : ""}`;
const scenarioHrefByName = (name) => scenarioHref(`name:${name}`);

// A tab's filters cleared except the ones in set, from the first rows and the top.
function resetTab(tab, set = {}) {
  ui.list.filters[tab] = { ...structuredClone(NO_FILTERS), ...set };
  delete ui.list.limit[tab];
  ui.list.scroll[tab] = 0;
}

// Opens a Scenarios tab with its filters cleared except those in set, for example
// { skill } or { played: "yes" } (Today's links).
function openScenariosTab(tab, set = {}) {
  resetTab(tab, set);
  const hash = `#scenarios/${tab}`;
  if (location.hash === hash) render();
  else location.hash = hash;
}

function playingWith() {
  const ps = ui.trainer.data.profiles;
  const p = ps.profiles.find((x) => x.name === ps.active) ?? ps.profiles[0];
  return `<a class="chip-link" href="#settings/aim" title="${esc(sensText(p))} · FOV ${num(p.settings.fov)}. Change it in Settings."><span class="muted">Playing with</span> <strong>${esc(p.name)}</strong> <span>${cmText(profileCm(p))} cm/360 · FOV ${num(p.settings.fov)}</span></a>`;
}

// The adaptive mix in use, linking to where it is set. For trainer scenarios and
// workouts only: a KovaaK's scenario has no mix.
function mixChip() {
  return `<a class="chip-link" href="#settings/training" title="How adaptive runs mix target difficulty. Change it in Settings."><span class="muted">Mix</span> <strong>${esc(mixName(ui.trainer.data.training.mix))}</strong></a>`;
}

// Shown on Scenarios and Today: the trainer is open or closed with an error, or a
// message from the last action.
function trainerBanners() {
  const tr = ui.trainer;
  return `${trainerErrorBanner()}
    ${tr.data?.playing ? `<section class="banner info"><div><div class="banner-title">The trainer is open</div><div class="banner-body">Finish or press Esc to come back. Each run is saved when its timer ends.</div></div></section>` : ""}
    ${tr.message ? `<section class="banner"><div class="banner-body">${esc(tr.message)}</div></section>` : ""}`;
}

function scenariosPage(arg, tab) {
  const tr = ui.trainer;
  if (tr.error) return `<div class="page">${head("Play, and see how you're doing", "Scenarios")}<div class="card error">${esc(tr.error)}</div></div>`;
  const wait = waitFor([trainerNeed(), overviewNeed()], "Loading scenarios…");
  if (wait) return wait;
  const all = scenarioItems();
  if (arg?.startsWith("name:")) {
    // Links from Today and older pages name a scenario. Swap to its own address,
    // so Back skips this hop. A tab in the address goes with it.
    const n = arg.slice(5);
    const item = all.find((x) => x.name === n && x.kind !== "workout");
    location.replace(item ? scenarioHref(item.key, tab) : "#scenarios");
    return "";
  }
  if (!arg) {
    location.replace(`#scenarios/${defaultTab()}`);
    return "";
  }
  if (Object.hasOwn(OLD_TABS, arg)) {
    // The old Recent and Yours addresses: Browse with that filter. Swap the address,
    // so Back doesn't land on it again.
    resetTab("browse", OLD_TABS[arg]);
    location.replace("#scenarios/browse");
    return "";
  }
  if (arg === "program") {
    ui.list.tab = arg;
    return programPage(all);
  }
  if (isListTab(arg)) {
    ui.list.tab = arg;
    return listPage(all, arg);
  }
  return itemPage(all.find((x) => x.key === arg) ?? null, all, tab);
}

function emptyTab(tab) {
  if (tab === "hidden") return `Nothing hidden. Hide something from its page to keep it off the lists.`;
  return "Nothing here yet.";
}

// What a list says when its filters leave nothing. The Clear filters button is above it.
function noMatches(tab, base) {
  const f = listFilters(tab);
  if (f.owner === "mine" && !base.some((x) => x.mine)) return "Nothing of yours yet. Use Copy &amp; edit on any scenario, or New scenario above. Clear the filters above to see everything.";
  if (f.played === "yes" && !base.some((x) => x.last)) return "Nothing played yet. Clear the filters above to see every scenario you can play.";
  return "Nothing matches these filters.";
}

const option = (v, label, sel) => `<option value="${esc(v)}" ${sel ? "selected" : ""}>${esc(label)}</option>`;
const filterSelect = (key, label, options, value) =>
  `<label class="lib-filter"><span>${label}</span><select class="select" data-list-filter="${key}">${options.map(([v, l]) => option(v, l, value === v)).join("")}</select></label>`;

// The search and the tab's filters. A filter only shows when it would split the
// tab: Skill with one skill, or Type with one kind, would do nothing.
function listTools(tab, base) {
  const f = listFilters(tab);
  const on = TAB_SETUP[tab].filters;
  const parts = [];
  if (on.includes("kind")) {
    const kinds = [["trainer", "AimAdapt"], ["kovaaks", "KovaaK's"], ["workout", "Workouts"]].filter(([k]) => base.some((x) => x.kind === k));
    if (base.some((x) => x.sce?.playable)) kinds.push(["kovaaks-play", "KovaaK's that play here"]);
    if (kinds.length > 1 || f.kind) parts.push(filterSelect("kind", "Type", [["", "All"], ...kinds], f.kind));
  }
  if (on.includes("skill")) {
    const skills = [...new Set(base.map((x) => x.skill).filter(Boolean))].sort((a, b) => skillOrder(a) - skillOrder(b) || a.localeCompare(b));
    if (skills.length > 1 || f.skill) parts.push(filterSelect("skill", "Skill", [["", "All skills"], ...skills.map((s) => [s, s])], f.skill));
  }
  if (on.includes("played")) parts.push(filterSelect("played", "Played", [["", "All"], ["yes", "Played"], ["no", "Not played yet"]], f.played));
  if (on.includes("owner")) parts.push(filterSelect("owner", "Made by", [["", "Anyone"], ["builtin", "Built-in"], ["mine", "You"]], f.owner));
  const favCount = tab === "hidden" ? 0 : base.filter((x) => isFavorite(x.key)).length;
  if (favCount || f.favorites) {
    parts.push(`<button type="button" class="chip fav-chip ${f.favorites ? "on" : ""}" data-action="fav-only" aria-pressed="${f.favorites}">${starIcon(f.favorites)}Favorites <span class="dim">${favCount}</span></button>`);
  }
  const mineOn = f.owner === "mine";
  const mineCount = base.filter((x) => x.mine).length;
  if (on.includes("mine") && (mineCount || mineOn)) {
    parts.push(`<button type="button" class="chip fav-chip ${mineOn ? "on" : ""}" data-action="yours-only" aria-pressed="${mineOn}">Yours <span class="dim">${mineCount}</span></button>`);
  }
  return `<div class="lib-tools">
    <label for="list-q" class="sr-only">Search</label>
    <div class="field lib-search"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9aa1ac" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"></circle><path d="M20 20l-4-4"></path></svg>
      <input id="list-q" type="search" placeholder="${tab === "workouts" ? "Search names and descriptions" : "Search names, skills and tags"}" value="${esc(f.query)}" autocomplete="off"></div>
    ${parts.join("")}
  </div>`;
}

// Tag chips, most used first. Picking several shows only what has all of them.
function tagChips(tab, base) {
  if (!TAB_SETUP[tab].filters.includes("tags")) return "";
  const f = listFilters(tab);
  const count = new Map();
  for (const x of base) for (const t of x.tags) count.set(t, (count.get(t) ?? 0) + 1);
  const tags = [...count.keys()].sort((a, b) => count.get(b) - count.get(a) || a.localeCompare(b));
  if (!tags.length) return "";
  const visible = ui.list.moreTags ? tags : tags.filter((t, i) => i < TAGS_SHOWN || f.tags.includes(t));
  return `<div class="tag-chips" role="group" aria-label="Tags"><span class="small muted">Tags</span>${visible.map((t) => `<button type="button" class="chip ${f.tags.includes(t) ? "on" : ""}" data-action="list-tag" data-tag="${esc(t)}" aria-pressed="${f.tags.includes(t)}">${esc(t)} <span class="dim">${count.get(t)}</span></button>`).join("")}
    ${tags.length > TAGS_SHOWN ? `<button type="button" class="link-btn" data-action="more-tags">${ui.list.moreTags ? "Fewer tags" : `${tags.length - visible.length} more`}</button>` : ""}</div>`;
}

function listTable(tab, rows, sort) {
  const [sortCol, sortDir] = sort;
  const cols = TAB_SETUP[tab].cols.map((id) => [id, COLUMNS[id]]);
  const limit = ui.list.limit[tab] ?? PAGE_ROWS;
  const th = ([id, c]) => {
    const sortable = Boolean(c.key || c.cmp);
    const on = sortable && sortCol === id;
    const inner = sortable
      ? `<button type="button" class="sort-btn" data-action="sort" data-col="${id}" title="${esc(c.title ?? `Sort by ${c.label.toLowerCase()}`)}">${c.label}<span class="sort-arrow" aria-hidden="true">${on ? (sortDir > 0 ? "▲" : "▼") : ""}</span></button>`
      : c.sr ? `<span class="sr-only">${c.sr}</span>` : c.label;
    return `<th class="c-${id} ${c.cls ?? ""}" ${on ? `aria-sort="${sortDir > 0 ? "ascending" : "descending"}"` : ""} scope="col">${inner}</th>`;
  };
  const left = rows.length - limit;
  const group = COLUMNS[sortCol].group;
  const sizes = new Map();
  if (group) for (const x of rows) sizes.set(group(x), (sizes.get(group(x)) ?? 0) + 1);
  let lastGroup = null;
  const body = rows.slice(0, limit).map((x) => {
    let heading = "";
    if (group && group(x) !== lastGroup) {
      lastGroup = group(x);
      heading = `<tr class="group-row"><th colspan="${cols.length}" scope="colgroup">${esc(lastGroup)} <span class="dim">${sizes.get(lastGroup)}</span></th></tr>`;
    }
    return `${heading}<tr>${cols.map(([id, c]) => `<td class="c-${id} ${c.cls ?? ""}">${c.cell(x, tab)}</td>`).join("")}</tr>`;
  });
  return `<div class="table-wrap"><table class="lib-table"><thead><tr>${cols.map(th).join("")}</tr></thead><tbody>
      ${body.join("")}
    </tbody></table></div>
    ${left > 0 ? `<div class="lib-more"><span class="small muted">Showing ${limit} of ${rows.length}</span>
      <button class="btn" type="button" data-action="more-rows">Show ${Math.min(PAGE_ROWS, left)} more</button>
      <button class="link-btn" type="button" data-action="all-rows">Show all</button></div>` : ""}`;
}

// The page head of every Scenarios tab: the title and the New workout and New scenario buttons.
function scenariosHead() {
  return head("Play, and see how you're doing", "Scenarios", `<span class="chips">${playingWith()}
      <button class="btn" type="button" data-action="new-workout">New workout</button>
      <button class="btn" type="button" data-action="new-scenario">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"></path></svg>New scenario</button></span>`);
}

// The tab row, with Hidden (and how many) once something is hidden.
function scenarioTabList(all, tab) {
  const hidden = all.filter((x) => isHidden(x.key)).length;
  return hidden || tab === "hidden" ? [...SCENARIO_TABS, ["hidden", `Hidden <span class="tab-count">${hidden}</span>`]] : SCENARIO_TABS;
}

// Scenarios › Program: the same head and tabs as a list, then the plan (programTab).
function programPage(all) {
  return `<div class="page">
    ${scenariosHead()}
    ${trainerBanners()}
    ${tabs("scenarios", scenarioTabList(all, "program"), "program")}
    ${programTab()}
  </div>`;
}

function listPage(all, tab) {
  const base = tabItems(all, tab);
  const sort = sortOf(tab, base);
  const rows = sortRows(filterRows(base, tab), sort);
  const filtering = isFiltering(tab);
  const noun = tab === "workouts" ? "workout" : tab === "browse" ? "scenario" : "item";
  const reading = ui.sceLibrary === "loading" && tab === "browse" ? " · Reading your KovaaK's scenarios…" : "";
  const count = (filtering ? `${rows.length} of ${base.length} match` : `${base.length} ${noun}${base.length === 1 ? "" : "s"} · ${TAB_SETUP[tab].note}`) + reading;
  return `<div class="page">
    ${scenariosHead()}
    ${trainerBanners()}
    ${tabs("scenarios", scenarioTabList(all, tab), tab)}
    <section class="card library" aria-label="${esc(TAB_LABELS[tab])}">
      ${tab === "workouts" ? `<div class="small muted">The Core Week's daily sessions are on the <a href="#scenarios/program">Program</a> tab.</div>` : ""}
      ${base.length ? `${listTools(tab, base)}${tagChips(tab, base)}
        <div class="lib-count small muted"><span>${esc(count)}</span>${filtering ? `<button type="button" class="link-btn" data-action="clear-list">Clear filters</button>` : ""}</div>
        ${rows.length ? listTable(tab, rows, sort) : `<div class="note-body">${noMatches(tab, base)}</div>`}`
        : `<div class="note-body">${emptyTab(tab)}</div>`}
    </section>
  </div>`;
}

// One scenario's or workout's own page, with a link back to the list you came from.
function itemPage(item, all, tab) {
  const listTab = defaultTab();
  const link = typeof helpLink === "function" ? helpLink() : "";
  return `<div class="page">
    <div class="back-row"><a class="back-link" href="#scenarios/${listTab}"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 18l-6-6 6-6"></path></svg>Scenarios · ${esc(TAB_LABELS[listTab])}</a>
      <div class="head-actions">${playingWith()}${item && item.kind !== "kovaaks" ? mixChip() : ""}${link}</div></div>
    ${trainerBanners()}
    ${item ? `<div class="detail" id="scenario-detail">${itemDetail(item, tab)}</div>`
      : `<div class="card note-body">This scenario or workout isn't here any more. <a href="#scenarios/${listTab}">Back to ${esc(TAB_LABELS[listTab])}</a></div>`}
  </div>`;
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

function itemDetail(x, tab) {
  if (x.kind === "workout") return workoutDetail(x.entry);
  if (x.kind === "kovaaks") return kovaaksDetail(x, tab);
  return trainerDetail(x.entry, tab);
}

// The tabs under an item's header. The tab asked for shows if the page has it, and
// Overview shows otherwise. Returns the tab's id with the row to draw.
function itemTabs(key, have, asked) {
  const list = ITEM_TABS.filter(([id]) => have.includes(id));
  const current = have.includes(asked) ? asked : "overview";
  return { current, row: tabs(`scenarios/${encodeURIComponent(key)}`, list, current, "overview") };
}

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
          <button class="btn primary" type="button" data-action="play-workout" data-id="${esc(e.id)}" ${playing} aria-label="Start session ${esc(w.name)}">${playIcon}Start session</button></span>
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

function trainerDetail(e, askedTab) {
  const s = e.scenario;
  const playing = ui.trainer.data.playing ? "disabled" : "";
  const played = (e.runs || 0) + (e.adaptive_runs || 0) > 0 || ui.insights.overview.some((o) => o.scenario === s.name);
  const data = played ? insightsFor(s.name) : undefined;
  const d = data && !data.error ? data : null;
  const adaptive = d ? ratedRuns(d) : [];
  const figures = d ? ratingLine(d, adaptive) : "";
  const header = `<section class="card detail-head">
      <div class="card-head" style="align-items:flex-start">
        <div><div class="chips"><span class="tag">AimAdapt</span>${e.builtin ? "" : '<span class="tag gold">Yours</span>'}${isHidden(e.id) ? '<span class="tag grey">Hidden</span>' : ""}<span class="small muted">${esc(SKILL_LABELS[s.skill] ?? s.category)}</span></div>
          <h2 class="detail-title">${esc(s.name)}</h2>
          <div class="note-body">${esc(s.description || "")}</div>
          <div class="small muted" style="margin-top:4px">${esc(scenarioFacts(s))}</div>
          ${figures ? `<div class="small" style="margin-top:4px">${esc(figures)}</div>` : ""}</div>
        <span class="chips">${markButtons(e.id)}
          <a class="btn" href="#edit/${encodeURIComponent(e.id)}">${e.builtin ? "Copy &amp; edit" : "Edit"}</a>
          <button class="btn" type="button" data-action="play" data-id="${esc(e.id)}" data-adaptive="0" ${playing}>Standard run</button>
          <span class="play-group"><button class="btn primary" type="button" data-action="play" data-id="${esc(e.id)}" data-adaptive="1" ${playing}>${playIcon}Play</button>
            <span class="small muted">adaptive · ${num(s.timing.duration_s)} s</span></span></span>
      </div>
    </section>`;
  if (!played) return `${header}<section class="card"><div class="note-body">Not played yet. Your first adaptive run is a calibration run that finds your level as you play; after that every run plays at one difficulty. See <a href="#help/adaptive">Adaptive mode</a>.</div></section>`;
  if (data === undefined) return `${header}<div class="empty-state">Loading…</div>`;
  if (data?.error) return `${header}<div class="card error">${esc(data.error)}</div>`;
  if (!d) return `${header}<section class="card"><div class="note-body">No detailed records for this scenario yet.</div></section>`;

  const { current, row } = itemTabs(e.id, ["overview", "insights", "runs"], askedTab);
  let body;
  if (current === "runs") {
    body = `${planCheckCard(d.plan)}<section class="card">${insightRunsTable(d)}</section>`;
  } else if (current === "insights") {
    const sections = [
      d.curve ? curveCard(d.curve) : "",
      d.fitts ? fittsCard(d.fitts) : "",
      d.directions.length >= 3 ? directionCard(d.directions) : "",
      d.shots ? shotCard(d.shots) : "",
      d.reaction ? reactionCard(d.reaction) : "",
      d.distances.length ? distanceCard(d.distances) : "",
      d.tracking ? trackingCard(d.tracking) : "",
      d.segments.some((x) => x.success !== null) ? segmentCard(d.segments) : "",
    ].filter(Boolean);
    body = `${workOnCard(d)}
      ${sections.length ? `<div class="insights-head"><h2>Insights</h2><span class="small muted">From your last ${d.recent_runs} runs</span></div><div class="insight-cols">${sections.join("")}</div>` : ""}`;
  } else {
    body = `<section class="kpis" aria-label="Key figures">${insightKpis(d, false).slice(0, 4).join("")}</section>
      ${adaptive.length ? ratingCard(d, adaptive) : ""}
      ${latestRunsCard(d, scenarioHref(e.id, "runs"))}
      ${workOnCard(d, 3, scenarioHref(e.id, "insights"))}`;
  }
  return `${header}${row}${body}`;
}

// Each KovaaK's scenario's .sce file and whether the trainer can play it, by name.
ui.sceInfo = new Map();

// "Play in AimAdapt" for a KovaaK's scenario whose file the trainer can play.
function sceCard(name) {
  const s = loadCached(ui.sceInfo, name, () => invoke("sce_info", { name }));
  if (!s || s.error || !s.path) return "";
  const playing = ui.trainer.data.playing ? "disabled" : "";
  const history = s.runs
    ? `In AimAdapt: ${s.runs} run${s.runs === 1 ? "" : "s"}, best ${num(s.best)}${s.last_played ? `, last ${fmtShort(dt(s.last_played))}` : ""}.`
    : "";
  if (!s.playable) {
    return `<section class="card"><div class="note-body"><strong>Can't play this in AimAdapt yet.</strong> It uses ${esc(s.reasons.join("; "))}. See <a href="#help/kovaaks-play">Playing KovaaK's scenarios</a>.</div>${history ? `<div class="small muted">${esc(history)}</div>` : ""}</section>`;
  }
  return `<section class="card sce-card"><div class="card-head" style="align-items:flex-start">
      <div><h2>In AimAdapt's trainer</h2>
        <div class="note-body">Plays this scenario's own file on its map in AimAdapt's trainer, in standard mode. Its runs are kept apart from your KovaaK's runs.</div>
        ${s.notes.length ? `<ul class="small muted sce-notes">${s.notes.map((n) => `<li>${esc(n[0].toUpperCase() + n.slice(1))}.</li>`).join("")}</ul>` : ""}
        ${history ? `<div class="small muted">${esc(history)}</div>` : ""}</div>
      <button class="btn primary" type="button" data-action="play-sce" data-path="${esc(s.path)}" ${playing}>${playIcon}Play in AimAdapt</button>
    </div></section>`;
}

// A KovaaK's scenario: its score history from KovaaK's stats files, if you've
// played it there, and Play in AimAdapt if the trainer can play its file.
function kovaaksDetail(x, askedTab) {
  const name = x.name;
  const played = Boolean(x.entry);
  const detail = played ? scenarioDetail(name) : null;
  if (detail === undefined) return `<div class="empty-state">Loading ${esc(name)}…</div>`;
  // If its history couldn't be loaded, the page says so where the history would be.
  const cached = detail?.error ? null : detail;
  const key = x.key;
  const about = played
    ? "Played in KovaaK's. Its stats files record score, accuracy and kill times; shot placement, reaction, tracking and replays come from runs in AimAdapt's trainer."
    : "In your KovaaK's scenarios, not played there yet. Your scores show here once you play it in KovaaK's.";
  const header = `<section class="card detail-head">
      <div class="card-head" style="align-items:flex-start">
        <div><div class="chips"><span class="tag blue">KovaaK's</span>${isHidden(key) ? '<span class="tag grey">Hidden</span>' : ""}${x.skill ? `<span class="small muted">${esc(x.skill)}</span>` : ""}</div>
          <h2 class="detail-title">${esc(name)}</h2>
          ${x.description ? `<div class="note-body">${esc(x.description)}</div>` : ""}
          ${x.tags.length ? `<div class="small muted" style="margin-top:4px">${esc(x.tags.join(" · "))}</div>` : ""}</div>
        <span class="chips">${markButtons(key)}</span>
      </div>
      <div class="small muted">${about}</div>
    </section>`;
  const error = detail?.error ? `<div class="card error">Couldn't load its runs: ${esc(detail.error)}</div>` : "";
  if (!cached) return `${header}${sceCard(name)}${error}`;
  const { current, row } = itemTabs(key, ["overview", "runs"], askedTab);
  const body = current === "runs"
    ? scenarioRunsCard(cached)
    : `${sceCard(name)}${scoreHistoryCard(cached)}${scenarioNotesCard(cached)}`;
  return `${header}${row}${body}`;
}

async function startSce(path) {
  await launchTrainer(() => invoke("play_sce", { path }));
  await loadTrainer();
}

// The tab on screen (a list, or Program), or null on an item's page. Program has no
// filters or sort to act on, but it keeps its scroll position like the lists.
const shownTab = () => {
  const { view, arg } = route();
  return view === "scenarios" && isScenarioTab(arg) ? arg : null;
};

// Each list remembers where it was scrolled, so Back from an item's page lands on
// the row you opened. app.js scrolls to the top on every new address, then calls
// this.
function restoreListScroll() {
  const tab = shownTab();
  if (tab && ui.list.scroll[tab]) window.scrollTo(0, ui.list.scroll[tab]);
}

window.addEventListener("scroll", () => {
  const tab = shownTab();
  if (tab) ui.list.scroll[tab] = window.scrollY;
}, { passive: true });

// Redraws the list after a filter changes, from the first page of rows.
function refilter(tab) {
  delete ui.list.limit[tab];
  render();
}

document.addEventListener("click", (e) => {
  const btn = e.target.closest("[data-action]");
  if (!btn) return;
  const a = btn.dataset.action;
  const tab = shownTab();
  if (a === "favorite") toggleMark("favorites", btn.dataset.key);
  else if (a === "hide") toggleMark("hidden", btn.dataset.key);
  else if (a === "browse-skill") openScenariosTab("browse", { skill: btn.dataset.skill ?? "" });
  else if (a === "list-tab-open") openScenariosTab(btn.dataset.tab, btn.dataset.played ? { played: btn.dataset.played } : {});
  else if (a === "play-sce") startSce(btn.dataset.path);
  else if (!tab) return;
  else if (a === "clear-list") {
    ui.list.filters[tab] = structuredClone(NO_FILTERS);
    refilter(tab);
  } else if (a === "sort") {
    const col = btn.dataset.col;
    const [now, dir] = sortOf(tab, tabItems(scenarioItems(), tab));
    ui.list.sort[tab] = now === col ? [col, -dir] : [col, COLUMNS[col].dir ?? 1];
    render();
  } else if (a === "list-tag") {
    const f = listFilters(tab);
    const t = btn.dataset.tag;
    f.tags = f.tags.includes(t) ? f.tags.filter((x) => x !== t) : [...f.tags, t];
    refilter(tab);
  } else if (a === "more-tags") {
    ui.list.moreTags = !ui.list.moreTags;
    render();
  } else if (a === "fav-only") {
    const f = listFilters(tab);
    f.favorites = !f.favorites;
    refilter(tab);
  } else if (a === "yours-only") {
    const f = listFilters(tab);
    f.owner = f.owner === "mine" ? "" : "mine";
    refilter(tab);
  } else if (a === "more-rows") {
    ui.list.limit[tab] = (ui.list.limit[tab] ?? PAGE_ROWS) + PAGE_ROWS;
    render();
  } else if (a === "all-rows") {
    ui.list.limit[tab] = Infinity;
    render();
  }
});

document.addEventListener("input", (e) => {
  const tab = shownTab();
  if (e.target.id !== "list-q" || !tab) return;
  listFilters(tab).query = e.target.value;
  refilter(tab); // render() keeps the focus and caret in the search box
});

document.addEventListener("change", (e) => {
  const key = e.target.dataset.listFilter;
  const tab = shownTab();
  if (key && tab) {
    listFilters(tab)[key] = e.target.value;
    refilter(tab);
  }
});
